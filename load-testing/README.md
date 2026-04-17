# FASO DIGITALISATION — Load testing (k6)

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## Scénarios disponibles

| Scénario | VUs | Durée | Objectif | Threshold |
|----------|-----|-------|----------|-----------|
| `armageddon-smoke.ts` | 10 | 1 min | Sanity check | P99 < 50 ms |
| `armageddon-baseline.ts` | 100 | 5 min | SLO compliance | **P99 < 10 ms, error rate < 0.1%** |

## Exécution locale

```bash
# Prérequis : stack FASO démarrée
cd INFRA/docker/compose && podman-compose up -d

# Smoke
k6 run INFRA/load-testing/k6/scenarios/armageddon-smoke.ts

# Baseline (fail si P99 > 10ms)
k6 run INFRA/load-testing/k6/scenarios/armageddon-baseline.ts \
  --summary-export=summary.json

# Avec token auth
AUTH_TOKEN="$(curl -s http://localhost:8801/auth/login -d '...' | jq -r .token)" \
  k6 run ...
```

## CI

Workflow `.github/workflows/load-test.yml` :
- Déclenché sur PR modifiant `INFRA/armageddon/**` ou `INFRA/load-testing/**`
- Bloque la PR si P99 > 10 ms ou erreur > 0.1 %
- Uploade `summary.json` en artifact (30 j)

## Intégration Grafana Cloud K6 (optionnel)

```bash
K6_CLOUD_TOKEN=xxx k6 cloud INFRA/load-testing/k6/scenarios/armageddon-baseline.ts
```

## Roadmap

- `armageddon-stress.ts` (ramp 100→1000 VUs)
- `armageddon-spike.ts` (test rate limiter + WAF)
- scénarios par microservice : `auth-ms-login`, `poulets-order-flow`
- intégration bencher.dev pour tracking régression P99 continue
