# Postmortem automation

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## Flow

```
Prometheus rule → Alertmanager (critical) → postmortem-bot (webhook /alert)
                                            ├─ dedupe via KAYA SET NX EX 7j
                                            └─ POST /issues via GitHub API
                                                └─ labels: incident, P1, postmortem-required
```

## Déploiement

```bash
# Build postmortem-bot image
podman build -t faso/postmortem-bot -f INFRA/docker/images/Containerfile.postmortem-bot .
# Run
podman run -d --name postmortem-bot \
  -p 8084:8084 \
  -e GITHUB_TOKEN=$(cat ~/.gh-token) \
  -e GITHUB_REPO=fasodigit/infra \
  -e KAYA_URL=redis://kaya:6380 \
  faso/postmortem-bot
```

## Configuration Alertmanager

Le fichier `INFRA/observability/alertmanager/alertmanager.yml` route les alertes `severity=critical` vers `postmortem-bot`.

## Runbooks référencés par annotations `runbook_url`

- `runbooks/kaya-down.md`
- `runbooks/armageddon-high-latency.md`
- `runbooks/auth-ms-down.md`
- `runbooks/poulets-api-down.md`
- `runbooks/notifier-dlq-backlog.md`
- `runbooks/synthetic-failure.md`

## Template issue

Voir `postmortem-bot.py:ISSUE_TEMPLATE` — inclut :
- Détection (alertname, summary, labels, runbook)
- Actions immédiates (ack, mitigate, rollback)
- Investigation (root cause, timeline, impact)
- Post-mortem à 48h (5 Whys, corrective + preventive actions)

## Processus humain

1. Oncall reçoit notif Slack `#faso-oncall` + GitHub issue auto-créée
2. ACK via réaction Slack + comment issue "ACKed by @user"
3. Mitigation suivant runbook
4. Après résolution : bot commente "Resolved at ..."
5. Post-mortem meeting dans 48h (planifié manuellement)
6. Corrective actions → tickets GitHub projects avec owners

## Tests

```bash
cd INFRA/scripts
python -m pytest tests/test_postmortem_bot.py -v
```

Couvre : firing → issue created, dedup replay → no duplicate, resolved → comment, malformed payload → 400.
