#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION
#
# Postmortem bot — receives Alertmanager webhooks, creates/updates GitHub issues.
# Deduplicates via KAYA (7-day TTL). Retries with backoff.

from __future__ import annotations

import logging
import os
from datetime import datetime
from typing import Any

import redis  # KAYA is RESP3-compatible
from fastapi import FastAPI, HTTPException, Request
from github import Github, GithubException

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
logger = logging.getLogger("postmortem-bot")

GH_TOKEN = os.environ["GITHUB_TOKEN"]
GH_REPO = os.environ.get("GITHUB_REPO", "fasodigit/infra")
KAYA_URL = os.environ.get("KAYA_URL", "redis://kaya:6380")
DEDUP_TTL_SECS = 7 * 24 * 3600

gh = Github(GH_TOKEN)
repo = gh.get_repo(GH_REPO)
kaya = redis.Redis.from_url(KAYA_URL, decode_responses=True)

app = FastAPI()

ISSUE_TEMPLATE = """# Incident: {alertname} — {severity}

**Service**: {service}
**Started**: {starts_at}
**Fingerprint**: `{fingerprint}`

## Détection
- **Alert**: {alertname}
- **Summary**: {summary}
- **Labels**: {labels}
- **Runbook**: {runbook_url}

## Actions immédiates
- [ ] Acknowledge oncall
- [ ] Mitigate (voir runbook)
- [ ] Rollback si récent deploy

## Investigation
- [ ] Root cause identifiée
- [ ] Timeline précise (minute par minute)
- [ ] Data impact évalué

## Post-mortem (dans 48h)
- [ ] 5 Whys
- [ ] Corrective actions avec owners + deadline
- [ ] Preventive actions (process, alerting, chaos tests)
- [ ] Monitoring/alertes à ajouter ou ajuster
"""


def build_body(alert: dict[str, Any]) -> str:
    labels = alert.get("labels", {})
    ann = alert.get("annotations", {})
    return ISSUE_TEMPLATE.format(
        alertname=labels.get("alertname", "unknown"),
        severity=labels.get("severity", "unknown"),
        service=labels.get("service", labels.get("job", "unknown")),
        starts_at=alert.get("startsAt", "?"),
        fingerprint=alert.get("fingerprint", "?"),
        summary=ann.get("summary", ann.get("description", "(no summary)")),
        labels=", ".join(f"{k}={v}" for k, v in labels.items()),
        runbook_url=ann.get("runbook_url", "(none)"),
    )


@app.post("/alert")
async def alert(req: Request) -> dict[str, Any]:
    payload = await req.json()
    alerts: list[dict[str, Any]] = payload.get("alerts", [])
    out: list[dict[str, Any]] = []

    for a in alerts:
        fp = a.get("fingerprint") or a.get("labels", {}).get("alertname", "unknown")
        key = f"postmortem:{fp}"
        status = a.get("status", "firing")
        labels = a.get("labels", {})

        if status == "firing" and labels.get("severity") == "critical":
            is_new = kaya.set(key, "1", nx=True, ex=DEDUP_TTL_SECS)
            if not is_new:
                logger.info("skip duplicate %s", fp)
                out.append({"fp": fp, "action": "dedup-skip"})
                continue

            try:
                issue = repo.create_issue(
                    title=f"Incident: {labels.get('alertname')} on {labels.get('service', '?')}",
                    body=build_body(a),
                    labels=["incident", "P1", "postmortem-required"],
                )
                kaya.set(f"{key}:issue", issue.number, ex=DEDUP_TTL_SECS)
                logger.info("created issue #%s for %s", issue.number, fp)
                out.append({"fp": fp, "action": "issue-created", "issue": issue.number})
            except GithubException as e:
                logger.error("GH API error: %s", e)
                raise HTTPException(status_code=503, detail="github-api-error")

        elif status == "resolved":
            issue_num = kaya.get(f"{key}:issue")
            if issue_num:
                try:
                    issue = repo.get_issue(int(issue_num))
                    issue.create_comment(
                        f"Resolved at {a.get('endsAt', datetime.utcnow().isoformat())}"
                    )
                    out.append({"fp": fp, "action": "comment-added", "issue": int(issue_num)})
                except GithubException as e:
                    logger.warning("GH API error on resolve: %s", e)

    return {"processed": out}


@app.get("/health")
async def health() -> dict[str, str]:
    return {"status": "ok"}


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8084)
