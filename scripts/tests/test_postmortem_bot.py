# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION
#
# Tests postmortem-bot — firing -> issue, dedup, resolved ignored, rate-limit retry.
from __future__ import annotations

import importlib.util
import os
import pathlib
import sys
import types

import httpx
import pytest
from fastapi.testclient import TestClient

HERE = pathlib.Path(__file__).resolve().parent
SCRIPTS = HERE.parent


def _load_bot_module() -> types.ModuleType:
    os.environ["POSTMORTEM_BOT_NO_STARTUP"] = "1"
    os.environ.setdefault("GITHUB_TOKEN", "test-token")
    os.environ.setdefault("GITHUB_REPO", "faso-digitalisation/INFRA")
    os.environ.setdefault("ONCALL_FILE", str(SCRIPTS.parent / "observability" / "oncall.yml"))
    spec = importlib.util.spec_from_file_location(
        "postmortem_bot", SCRIPTS / "postmortem-bot.py"
    )
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules["postmortem_bot"] = mod
    spec.loader.exec_module(mod)
    return mod


bot = _load_bot_module()


# --------------------------------------------------------------------------- #
# Fake GH client + transport
# --------------------------------------------------------------------------- #


class FakeGH:
    """Drop-in async replacement for GitHubClient."""

    def __init__(self) -> None:
        self.existing: dict[str, int] = {}  # fingerprint -> issue number
        self.created: list[dict] = []
        self.comments: list[tuple[int, str]] = []
        self._next_num = 100

    async def aclose(self) -> None:
        return None

    async def search_existing_issue(self, fingerprint: str) -> int | None:
        return self.existing.get(fingerprint)

    async def create_issue(self, title, body, labels, assignees):
        self._next_num += 1
        num = self._next_num
        self.created.append(
            {"title": title, "body": body, "labels": labels, "assignees": assignees, "number": num}
        )
        # Register for later dedup.
        for line in body.splitlines():
            if "postmortem-fingerprint:" in line:
                fp = line.split("postmortem-fingerprint:")[1].strip().rstrip("-->").strip()
                self.existing[fp] = num
        return num

    async def comment_issue(self, number: int, body: str) -> None:
        self.comments.append((number, body))


@pytest.fixture
def client():
    fake = FakeGH()
    bot.state.gh = fake
    with TestClient(bot.app) as c:
        yield c, fake
    bot.state.gh = None


# --------------------------------------------------------------------------- #
# Payloads
# --------------------------------------------------------------------------- #

FIRING_PAYLOAD = {
    "version": "4",
    "status": "firing",
    "alerts": [
        {
            "status": "firing",
            "labels": {
                "alertname": "KayaNodeDown",
                "severity": "critical",
                "team": "sre",
                "service": "kaya",
                "instance": "kaya-0.kaya.faso.svc:6380",
                "cluster": "prod",
            },
            "annotations": {
                "summary": "KAYA node 0 unreachable for 5m",
                "runbook_url": "https://runbooks.faso/observability/kaya-node-down",
                "grafana_url": "https://grafana.faso/d/kaya",
            },
            "startsAt": "2026-04-18T05:00:00Z",
            "endsAt": "0001-01-01T00:00:00Z",
        }
    ],
}


RESOLVED_PAYLOAD = {
    "alerts": [
        {
            "status": "resolved",
            "labels": {"alertname": "KayaNodeDown", "severity": "critical"},
            "annotations": {},
            "startsAt": "2026-04-18T05:00:00Z",
            "endsAt": "2026-04-18T05:30:00Z",
        }
    ],
}


# --------------------------------------------------------------------------- #
# Tests
# --------------------------------------------------------------------------- #


def test_healthz(client):
    c, _ = client
    r = c.get("/healthz")
    assert r.status_code == 200
    assert r.json() == {"ok": True}


def test_firing_creates_issue(client):
    c, fake = client
    r = c.post("/alert", json=FIRING_PAYLOAD)
    assert r.status_code == 200, r.text
    data = r.json()
    assert data["processed"][0]["action"] == "create"
    assert len(fake.created) == 1
    issue = fake.created[0]
    assert "[POSTMORTEM]" in issue["title"]
    assert "KayaNodeDown" in issue["title"]
    assert "postmortem" in issue["labels"]
    assert "severity:critical" in issue["labels"]
    assert "component:kaya" in issue["labels"]
    assert "postmortem-fingerprint:" in issue["body"]


def test_firing_same_fingerprint_deduplicates(client):
    c, fake = client
    r1 = c.post("/alert", json=FIRING_PAYLOAD)
    r2 = c.post("/alert", json=FIRING_PAYLOAD)
    assert r1.status_code == 200 and r2.status_code == 200
    assert r2.json()["processed"][0]["action"] == "comment"
    assert len(fake.created) == 1
    assert len(fake.comments) == 1
    num, body = fake.comments[0]
    assert num == fake.created[0]["number"]
    assert "still active" in body.lower()


def test_resolved_alert_ignored(client):
    c, fake = client
    r = c.post("/alert", json=RESOLVED_PAYLOAD)
    assert r.status_code == 200
    assert r.json()["processed"][0]["action"] == "resolved-skip"
    assert fake.created == []
    assert fake.comments == []


def test_invalid_json_400(client):
    c, _ = client
    r = c.post("/alert", content=b"not-json", headers={"Content-Type": "application/json"})
    assert r.status_code == 400


def test_fingerprint_stable():
    labels = {
        "alertname": "HighErrorRate",
        "service": "auth-ms",
        "instance": "auth-ms-7",
        "cluster": "prod",
        "ignored": "should-not-affect",
    }
    fp1 = bot.compute_fingerprint(labels)
    fp2 = bot.compute_fingerprint({**labels, "ignored": "changed"})
    assert fp1 == fp2
    assert len(fp1) == 64


def test_oncall_lookup_fallback():
    # Week not in the rotation -> default_assignee.
    import datetime as dt

    got = bot.resolve_oncall(today=dt.date(2030, 1, 1))
    assert got == "sre-team-lead"


# --------------------------------------------------------------------------- #
# GH API rate-limit retry (real GitHubClient against fake transport)
# --------------------------------------------------------------------------- #


@pytest.mark.asyncio
async def test_rate_limit_retry_then_success(monkeypatch):
    """429 once, then 201 — GitHubClient must retry and succeed."""
    calls = {"n": 0}

    def handler(request: httpx.Request) -> httpx.Response:
        calls["n"] += 1
        if calls["n"] == 1:
            return httpx.Response(
                429,
                headers={"x-ratelimit-reset": "0"},
                json={"message": "API rate limit exceeded"},
            )
        return httpx.Response(201, json={"number": 42})

    transport = httpx.MockTransport(handler)

    async def _no_sleep(_s):  # keep tests fast
        return None

    monkeypatch.setattr(bot.asyncio, "sleep", _no_sleep)
    gh = bot.GitHubClient(token="t", repo="faso-digitalisation/INFRA")
    await gh._client.aclose()
    gh._client = httpx.AsyncClient(base_url=bot.GH_API, transport=transport, timeout=5.0)
    num = await gh.create_issue("t", "b", ["postmortem"], [])
    assert num == 42
    assert calls["n"] == 2
    await gh.aclose()


@pytest.mark.asyncio
async def test_rate_limit_retries_exhausted(monkeypatch):
    """Persistent 429 -> HTTPException 503."""

    def handler(_r: httpx.Request) -> httpx.Response:
        return httpx.Response(
            429,
            headers={"x-ratelimit-reset": "0"},
            json={"message": "API rate limit exceeded"},
        )

    transport = httpx.MockTransport(handler)

    async def _no_sleep(_s):
        return None

    monkeypatch.setattr(bot.asyncio, "sleep", _no_sleep)
    gh = bot.GitHubClient(token="t", repo="faso-digitalisation/INFRA")
    await gh._client.aclose()
    gh._client = httpx.AsyncClient(base_url=bot.GH_API, transport=transport, timeout=5.0)
    from fastapi import HTTPException

    with pytest.raises(HTTPException) as exc:
        await gh.create_issue("t", "b", ["postmortem"], [])
    assert exc.value.status_code == 503
    await gh.aclose()
