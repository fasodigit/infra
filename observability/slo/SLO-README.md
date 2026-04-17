# FASO DIGITALISATION — SLOs (Sloth)

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## Méthodologie

Les SLOs (Service Level Objectives) sont définis **as code** en YAML format Sloth.
Sloth génère automatiquement les recording rules + alerting rules Prometheus conformes
aux best practices Google SRE (multi-burn-rate alerting).

## Services couverts

| Service | Availability | Latency objective | Error budget |
|---------|--------------|-------------------|--------------|
| KAYA | 99.95% | 99% P99 < 1 ms | ~22 min / mois |
| ARMAGEDDON | 99.99% | 99% P99 < 10 ms | ~4.4 min / mois |
| auth-ms | 99.9% | 99% JWT validation < 5 ms | ~44 min / mois |
| poulets-platform | 99.5% | 99% GraphQL success | ~3h40 / mois |
| notifier-ms | 99% mail delivery | 99% lag P95 < 1000 | ~7h / mois |

## Workflow

```
slo/*.slo.yaml → scripts/generate-prometheus-rules.sh → prometheus/rules/*-slo-rules.yaml
                                                       → Prometheus scrape → alertmanager → postmortem-bot
                                                       → Grafana dashboard faso-sli-slo
```

## Procédure révision

- **Trimestrielle** : revue des objectifs par service owner + SRE lead
- **Sur incident** : si error budget consommé > 50% → post-mortem + amélioration requise
- **Sur release majeure** : recalibration si hypothèses de trafic changent

## Références

- RPO/RTO par projet dans `INFRA/docs/v3.1-souverain/rpo-rto/MATRICE-RPO-RTO-v3.1.md`
- Runbooks : `INFRA/observability/alertmanager/runbooks/`
- Dashboard Grafana : `faso-sli-slo` (uid versionné, auto-provisioned)
