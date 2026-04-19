#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION
#
# Postmortem bot — Phase 7 axe 19.
# Ecoute les webhooks Alertmanager, cree ou met a jour des GitHub issues
# a partir du template Google-SRE. Secrets via Vault. Dedup via fingerprint
# SHA256 + GH search API. Rate-limit retry avec backoff exponentiel.
from __future__ import annotations

import asyncio
import datetime as dt
import hashlib
import json
import logging
import os
import pathlib
import sys
from typing import Any

import httpx
import yaml
from fastapi import FastAPI, HTTPException, Request
from prometheus_client import Counter, Histogram, make_asgi_app
from pydantic import BaseModel

# --------------------------------------------------------------------------- #
# Logging: JSON structure
# --------------------------------------------------------------------------- #


class JsonFormatter(logging.Formatter):
    def format(self, record: logging.LogRecord) -> str:
        payload: dict[str, Any] = {
            "ts": dt.datetime.now(dt.timezone.utc).isoformat(),
            "level": record.levelname,
            "logger": record.name,
            "msg": record.getMessage(),
        }
        for k in ("alert_fingerprint", "issue_number", "action", "alertname"):
            v = getattr(record, k, None)
            if v is not None:
                payload[k] = v
        if record.exc_info:
            payload["exc"] = self.formatException(record.exc_info)
        return json.dumps(payload, ensure_ascii=False)


_handler = logging.StreamHandler(sys.stdout)
_handler.setFormatter(JsonFormatter())
logging.basicConfig(level=os.environ.get("LOG_LEVEL", "INFO"), handlers=[_handler])
logger = logging.getLogger("postmortem-bot")

# --------------------------------------------------------------------------- #
# Config & constants
# --------------------------------------------------------------------------- #

GH_REPO = os.environ.get("GITHUB_REPO", "faso-digitalisation/INFRA")
GH_API = "https://api.github.com"
VAULT_ADDR = os.environ.get("VAULT_ADDR", "http://vault:8200")
VAULT_TOKEN = os.environ.get("VAULT_TOKEN", "")
VAULT_PATH = os.environ.get("VAULT_GH_TOKEN_PATH", "faso/postmortem-bot/github-token")

ROOT = pathlib.Path(__file__).resolve().parent
TEMPLATE_PATH = ROOT / "templates" / "POSTMORTEM.md"
ONCALL_PATH = pathlib.Path(
    os.environ.get(
        "ONCALL_FILE",
        str(ROOT.parent / "observability" / "oncall.yml"),
    )
)

MAX_RETRIES = 5
BACKOFF_BASE_SECS = 1.0

# --------------------------------------------------------------------------- #
# Metrics
# --------------------------------------------------------------------------- #

ALERTS_RECEIVED = Counter(
    "postmortem_alerts_received_total", "Alerts received", ["status", "severity"]
)
ISSUES_CREATED = Counter("postmortem_issues_created_total", "GH issues created")
ISSUES_COMMENTED = Counter("postmortem_issues_commented_total", "GH issues commented")
GH_API_ERRORS = Counter("postmortem_github_errors_total", "GH API errors", ["kind"])
GH_API_LATENCY = Histogram("postmortem_github_latency_seconds", "GH API latency")

# --------------------------------------------------------------------------- #
# Secret loading (Vault KV v2)
# --------------------------------------------------------------------------- #


async def load_github_token() -> str:
    """Resolve the GH token from Vault (KV v2). Fallback to env GITHUB_TOKEN for dev/tests."""
    env_token = os.environ.get("GITHUB_TOKEN")
    if env_token:
        return env_token
    if not VAULT_TOKEN:
        raise RuntimeError("Vault unreachable and GITHUB_TOKEN not set")
    url = f"{VAULT_ADDR}/v1/{VAULT_PATH.replace('faso/', 'faso/data/', 1)}"
    async with httpx.AsyncClient(timeout=5.0) as c:
        r = await c.get(url, headers={"X-Vault-Token": VAULT_TOKEN})
    r.raise_for_status()
    return r.json()["data"]["data"]["token"]


# --------------------------------------------------------------------------- #
# Helpers
# --------------------------------------------------------------------------- #


def compute_fingerprint(labels: dict[str, str]) -> str:
    """Stable SHA256 on sorted (alertname, service, instance, cluster)."""
    material = {k: labels.get(k, "") for k in ("alertname", "service", "instance", "cluster")}
    blob = json.dumps(material, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(blob.encode()).hexdigest()


def iso_week_key(date: dt.date) -> str:
    y, w, _ = date.isocalendar()
    return f"{y}-W{w:02d}"


def resolve_oncall(today: dt.date | None = None) -> str:
    today = today or dt.datetime.now(dt.timezone.utc).date()
    try:
        with ONCALL_PATH.open() as f:
            cfg = yaml.safe_load(f) or {}
    except FileNotFoundError:
        return ""
    wanted = iso_week_key(today)
    for entry in cfg.get("rotation", []):
        if entry.get("week") == wanted:
            return str(entry.get("engineer", "")).lstrip("@")
    return str(cfg.get("default_assignee", "")).lstrip("@")


def load_template() -> str:
    return TEMPLATE_PATH.read_text(encoding="utf-8")


def render_body(alert: dict[str, Any], fingerprint: str, assignee: str) -> str:
    labels = alert.get("labels", {})
    ann = alert.get("annotations", {})
    tpl = load_template()
    fills = {
        "ALERT_NAME": labels.get("alertname", "unknown"),
        "SERVICE": labels.get("service", labels.get("job", "unknown")),
        "SEVERITY": labels.get("severity", "unknown"),
        "TIMESTAMP_UTC": alert.get("startsAt", dt.datetime.now(dt.timezone.utc).isoformat()),
        "FINGERPRINT": fingerprint,
        "ASSIGNEE": f"@{assignee}" if assignee else "_unassigned_",
        "GRAFANA_URL": ann.get("grafana_url", "(missing)"),
        "RUNBOOK_URL": ann.get("runbook_url", "(missing)"),
        "SUMMARY": ann.get("summary", ann.get("description", "(no summary)")),
    }
    for k, v in fills.items():
        tpl = tpl.replace("{{" + k + "}}", str(v))
    # Always include fingerprint marker so dedup search can find it.
    tpl += f"\n\n<!-- postmortem-fingerprint:{fingerprint} -->\n"
    return tpl


# --------------------------------------------------------------------------- #
# GitHub client with rate-limit retry
# --------------------------------------------------------------------------- #


class GitHubClient:
    def __init__(self, token: str, repo: str) -> None:
        self._token = token
        self._repo = repo
        self._client = httpx.AsyncClient(
            base_url=GH_API,
            headers={
                "Authorization": f"Bearer {token}",
                "Accept": "application/vnd.github+json",
                "X-GitHub-Api-Version": "2022-11-28",
            },
            timeout=10.0,
        )

    async def aclose(self) -> None:
        await self._client.aclose()

    async def _request(self, method: str, path: str, **kw: Any) -> httpx.Response:
        for attempt in range(MAX_RETRIES):
            with GH_API_LATENCY.time():
                r = await self._client.request(method, path, **kw)
            if r.status_code in (403, 429) and "rate limit" in r.text.lower():
                reset = int(r.headers.get("x-ratelimit-reset", "0"))
                now = int(dt.datetime.now(dt.timezone.utc).timestamp())
                wait = max(1, reset - now) if reset else BACKOFF_BASE_SECS * (2**attempt)
                GH_API_ERRORS.labels(kind="rate_limit").inc()
                logger.warning("github rate limit, sleeping %.1fs", wait)
                await asyncio.sleep(min(wait, 60))
                continue
            if 500 <= r.status_code < 600:
                GH_API_ERRORS.labels(kind="server").inc()
                await asyncio.sleep(BACKOFF_BASE_SECS * (2**attempt))
                continue
            return r
        GH_API_ERRORS.labels(kind="retries_exhausted").inc()
        raise HTTPException(status_code=503, detail="github-unreachable")

    async def search_existing_issue(self, fingerprint: str) -> int | None:
        q = f'repo:{self._repo} is:issue label:postmortem in:body "postmortem-fingerprint:{fingerprint}"'
        r = await self._request("GET", "/search/issues", params={"q": q})
        if r.status_code != 200:
            return None
        items = r.json().get("items", [])
        return items[0]["number"] if items else None

    async def create_issue(
        self,
        title: str,
        body: str,
        labels: list[str],
        assignees: list[str],
    ) -> int:
        payload = {"title": title, "body": body, "labels": labels, "assignees": assignees}
        r = await self._request("POST", f"/repos/{self._repo}/issues", json=payload)
        if r.status_code not in (200, 201):
            GH_API_ERRORS.labels(kind=f"create_{r.status_code}").inc()
            raise HTTPException(status_code=502, detail=f"gh-create-failed:{r.status_code}")
        return int(r.json()["number"])

    async def comment_issue(self, number: int, body: str) -> None:
        r = await self._request(
            "POST", f"/repos/{self._repo}/issues/{number}/comments", json={"body": body}
        )
        if r.status_code not in (200, 201):
            GH_API_ERRORS.labels(kind=f"comment_{r.status_code}").inc()
            raise HTTPException(status_code=502, detail=f"gh-comment-failed:{r.status_code}")


# --------------------------------------------------------------------------- #
# FastAPI app
# --------------------------------------------------------------------------- #

app = FastAPI(title="faso-postmortem-bot", version="1.0.0")
app.mount("/metrics", make_asgi_app())


class _State:
    gh: GitHubClient | None = None


state = _State()


@app.on_event("startup")
async def _startup() -> None:
    if os.environ.get("POSTMORTEM_BOT_NO_STARTUP") == "1":
        return  # test mode: tests will inject their own client
    token = await load_github_token()
    state.gh = GitHubClient(token, GH_REPO)
    logger.info("postmortem-bot ready", extra={"action": "startup"})


@app.on_event("shutdown")
async def _shutdown() -> None:
    if state.gh is not None:
        await state.gh.aclose()


@app.get("/healthz")
async def healthz() -> dict[str, bool]:
    return {"ok": True}


class ProcessResult(BaseModel):
    processed: list[dict]


# Pydantic v2: forward-refs from `Any` in list params can leave the model
# "not fully defined" when loaded via importlib. Rebuild explicitly.
ProcessResult.model_rebuild(force=True)


async def process_alert(alert: dict[str, Any]) -> dict[str, Any]:
    labels = alert.get("labels", {})
    status = alert.get("status", "firing")
    ALERTS_RECEIVED.labels(status=status, severity=labels.get("severity", "unknown")).inc()

    if status != "firing":
        logger.info(
            "resolved alert ignored",
            extra={"action": "resolved-skip", "alertname": labels.get("alertname", "")},
        )
        return {"action": "resolved-skip"}

    fingerprint = compute_fingerprint(labels)
    short = fingerprint[:8]
    alertname = labels.get("alertname", "unknown")
    service = labels.get("service", labels.get("job", "unknown"))
    severity = labels.get("severity", "unknown")

    assignee = resolve_oncall()
    assignees = [assignee] if assignee else []

    assert state.gh is not None, "GitHub client not initialised"

    existing = await state.gh.search_existing_issue(fingerprint)
    if existing is not None:
        comment = (
            f"Incident still active at "
            f"{dt.datetime.now(dt.timezone.utc).isoformat()} "
            f"(fingerprint `{short}`)."
        )
        await state.gh.comment_issue(existing, comment)
        ISSUES_COMMENTED.inc()
        logger.info(
            "commented existing postmortem",
            extra={
                "alert_fingerprint": fingerprint,
                "issue_number": existing,
                "action": "comment",
                "alertname": alertname,
            },
        )
        return {
            "action": "comment",
            "issue": existing,
            "fingerprint": fingerprint,
        }

    title = f"[POSTMORTEM] {alertname} - {service} - {short}"
    body = render_body(alert, fingerprint, assignee)
    gh_labels = ["postmortem", f"severity:{severity}", f"component:{service}"]
    number = await state.gh.create_issue(title, body, gh_labels, assignees)
    ISSUES_CREATED.inc()
    logger.info(
        "created postmortem issue",
        extra={
            "alert_fingerprint": fingerprint,
            "issue_number": number,
            "action": "create",
            "alertname": alertname,
        },
    )
    return {"action": "create", "issue": number, "fingerprint": fingerprint}


@app.post("/alert")
async def alert_endpoint(req: Request) -> dict[str, Any]:
    if state.gh is None:
        raise HTTPException(status_code=503, detail="bot-not-ready")
    try:
        payload = await req.json()
    except json.JSONDecodeError as exc:
        raise HTTPException(status_code=400, detail="invalid-json") from exc

    # Alertmanager can send either {alerts:[...]} or a bare list.
    if isinstance(payload, dict):
        alerts = payload.get("alerts", [])
    elif isinstance(payload, list):
        alerts = payload
    else:
        raise HTTPException(status_code=400, detail="unexpected-payload")

    out: list[dict[str, Any]] = []
    for a in alerts:
        out.append(await process_alert(a))
    return {"processed": out}


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(
        "postmortem_bot:app",
        host="0.0.0.0",
        port=8090,
        log_config=None,
    )
