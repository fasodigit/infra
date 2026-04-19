<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# FASO DIGITALISATION — Load testing (k6)

Scénarios k6 TypeScript ciblant **ARMAGEDDON** (gateway Rust souverain, port 8080)
et **KAYA** (DB in-memory Rust souveraine, port 6380, RESP3).

## Scénarios disponibles

| Fichier | Cible | Scénarios | Thresholds |
|---------|-------|-----------|------------|
| `k6/scenarios/armageddon.ts` | ARMAGEDDON :8080 | smoke (10 VU/1 min), load (100 VU/5 min), stress (500 VU/10 min), spike (0→1000 VU/30 s) | P99 < 10 ms, P95 < 5 ms, erreur < 0.1 %, > 5 000 rps |
| `k6/scenarios/kaya-resp3.ts` | KAYA :6380 | RESP3 SET/GET/INCR/HSET/ZADD (70/15/10/3/2 %) | P99 < 1 ms, > 100 000 ops/s |
| `k6/scenarios/armageddon-smoke.ts` | ARMAGEDDON | legacy smoke historique | P99 < 50 ms |
| `k6/scenarios/armageddon-baseline.ts` | ARMAGEDDON | legacy baseline historique | P99 < 10 ms |

## Invocation locale

```bash
# 1. Démarrer la stack (podman-compose obligatoire)
cd INFRA/docker/compose
bash scripts/init-secrets.sh
podman-compose -f podman-compose.yml up -d kaya armageddon auth-ms poulets-api

# 2. Scénario ARMAGEDDON (par défaut : smoke)
k6 run INFRA/load-testing/k6/scenarios/armageddon.ts

# 3. Choisir un scénario (smoke | load | stress | spike)
K6_SCENARIO=load   k6 run INFRA/load-testing/k6/scenarios/armageddon.ts
K6_SCENARIO=stress k6 run INFRA/load-testing/k6/scenarios/armageddon.ts
K6_SCENARIO=spike  k6 run INFRA/load-testing/k6/scenarios/armageddon.ts

# 4. JWT personnalisé (sinon un token dev HS256 est généré à la volée)
AUTH_TOKEN="$(curl -s http://localhost:8801/auth/login \
  -d \"{\\\"password\\\":\\\"$E2E_TEST_PASSWORD\\\"}\" | jq -r .token)" \
  k6 run INFRA/load-testing/k6/scenarios/armageddon.ts

# 5. Scénario KAYA RESP3 — requiert k6 custom avec xk6-redis
go install go.k6.io/xk6/cmd/xk6@latest
xk6 build --with github.com/grafana/xk6-redis --output ./k6-redis
./k6-redis run INFRA/load-testing/k6/scenarios/kaya-resp3.ts
```

## Helpers partagés

`k6/lib/helpers.ts` fournit :

- `buildDevJwt(sub, extraClaims)` : JWT HS256 dev (clé `FASO_JWT_DEV_KEY`).
- `poulet1KB()` : payload JSON poulet ~1 KB (race, vaccinations, traçabilité).
- `etatCivilCertificateRequest()` : demande certificat état-civil (SOGESY fake).
- `kayaKey(prefix)`, `kayaSmallValue()` : générateurs clé/valeur KAYA.

## Tags k6 émis

`environment` (local|ci|staging) · `scenario` (smoke|load|stress|spike|kaya_resp3) ·
`endpoint` (healthz|poulets_post|etat_civil_get) · `service` (armageddon|kaya) · `cmd` (KAYA uniquement).

## CI GitHub Actions

Workflow `.github/workflows/load-test.yml` :

- Déclenché sur PR portant label `perf` ou `load-test`, plus cron dominical.
- Démarre la stack via **podman-compose** (aucune dépendance Docker Engine).
- Upload `summary-armageddon.json` + `summary-kaya.json` en artifact (30 j).
- **Bloque le merge** si P99 courant > baseline (`.github/k6-baseline.json`) + 10 %.

## Secrets requis

| Secret GH Actions | Usage |
|-------------------|-------|
| `E2E_TEST_PASSWORD` | Login auth-ms en CI |
| `FASO_JWT_DEV_KEY` | Signature JWT HS256 dev (scénarios) |

## Roadmap

- Intégration bencher.dev pour tracking régression P99 continue.
- Scénarios par microservice : `auth-ms-login`, `poulets-order-flow`, `notifier-broadcast`.
- Export Grafana Cloud k6 (`k6 cloud`) pour runs longs > 30 min.
