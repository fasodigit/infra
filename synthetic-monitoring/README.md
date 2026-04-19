<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 FASO DIGITALISATION -->

# FASO DIGITALISATION — Synthetic Monitoring (Playwright)

Phase 7 axe 16. Monitoring synthétique production via navigateur headless
(Playwright Chromium). Lance trois parcours utilisateur toutes les 5 minutes,
mesure les timings et pousse des métriques vers Prometheus Pushgateway.

## Arborescence

```
synthetic-monitoring/
├── playwright.config.ts              # configuration principale
├── package.json                      # scripts (bun run test:auth, …)
├── playwright/
│   ├── flows/
│   │   ├── auth.spec.ts              # login + logout < 3 s
│   │   ├── poulets-order.spec.ts     # browse + add-to-cart + checkout
│   │   └── etat-civil.spec.ts        # certificat de naissance (stub-safe)
│   ├── fixtures/
│   │   └── synthetic-user.ts         # compte dédié Vault
│   └── helpers/
│       ├── prometheus-push.ts        # client Pushgateway
│       └── timing.ts                 # FCP / LCP / TTI / HTTP 5xx
├── cron/
│   └── run-all.sh                    # loop 5 min, parallel, timeout 2 min
├── deploy/
│   └── Deployment.yaml               # K8s CronJob + ServiceAccount + NetPol
├── Dockerfile                        # image ghcr.io/.../synthetic-monitoring
└── README.md
```

## Flows et SLA

| Flow           | Parcours                                  | SLA cible |
|----------------|-------------------------------------------|-----------|
| `auth`         | landing → login → logout                  | < 3 s     |
| `poulets-order`| catalogue → login → add-to-cart → checkout| < 15 s    |
| `etat-civil`   | demande acte de naissance                 | < 10 s    |

## Métriques poussées

Job `synthetic_monitoring`, labels `flow`, `env`, `region`
(`ouagadougou-1` | `bobo-1`).

- `synthetic_duration_seconds` — durée totale
- `synthetic_success` — 0/1
- `synthetic_step_duration_seconds{step}` — par étape
- `synthetic_fcp_seconds`, `synthetic_lcp_seconds`, `synthetic_tti_seconds`
- `synthetic_http5xx_total` — compteur HTTP 5xx
- `synthetic_error_rate` — dérivé `1 - success`

## Exécution locale

```bash
cd INFRA/synthetic-monitoring
bun install
bunx playwright install chromium --with-deps

export FASO_PROD_URL=http://localhost:4801
export FASO_ENV=dev
export FASO_REGION=ouagadougou-1
export PROM_PUSHGATEWAY_URL=http://localhost:9091
# secrets compte dédié — Vault en prod :
#   vault kv get -field=email    faso/synthetic-monitoring/user
#   vault kv get -field=password faso/synthetic-monitoring/user
export SYNTHETIC_USER_EMAIL=synthetic-monitor@faso.gov.bf
export SYNTHETIC_USER_PASSWORD='***'

bun run test:auth
# → résultat console + métriques PUT sur Pushgateway
```

Ouvrir le rapport HTML local : `bun run report`.

## Déploiement production (K8s)

```bash
kubectl apply -f synthetic-monitoring/deploy/Deployment.yaml
kubectl -n synthetic-monitoring get cronjob synthetic-monitoring
```

Les credentials sont injectés par Vault Agent Injector
(annotations `vault.hashicorp.com/agent-inject-*`), la policy AppRole
`synthetic-monitoring` n'autorise la lecture que de
`faso/synthetic-monitoring/user`.

## Dashboard Grafana

Le dashboard `FASO — Synthetic Monitoring` est provisionné via
`INFRA/observability/grafana/provisioning/dashboards/synthetic-monitoring.json`
(UID `faso-synthetic-monitoring`). Panels :

1. **Success rate last 24h** — stat par flow/region, seuils 95/99 %.
2. **P95 duration trend** — timeseries P95 et moyenne glissante 5 min.
3. **Step duration breakdown** — table durée moyenne par étape.
4. **Web Vitals** — FCP / LCP / TTI par flow.
5. **Active synthetic alerts** — alertlist (règles Prometheus).
6. **HTTP 5xx observed last 1h** — compteur par flow.

## Règle d'alerte recommandée (exemple, `synthetic.rules.yaml`)

```yaml
- alert: SyntheticAuthFlowKO
  expr: avg_over_time(synthetic_success{flow="auth"}[15m]) < 0.7
  for: 0m
  labels: { severity: critical, team: platform }
  annotations:
    summary: "auth flow KO > 3 itérations consécutives"
    runbook: "INFRA/observability/alertmanager/runbooks/synthetic-failure.md"
```

## CI/CD

- Workflow `.github/workflows/synthetic-build.yml` : build + push image
  `ghcr.io/faso-digitalisation/synthetic-monitoring:latest` sur main +
  weekly cron. Dry-run validation du flow `auth` sur staging.
- Workflow `.github/workflows/synthetic-prod.yml` (existant) : run 5 min
  depuis GitHub-hosted runners (fallback si cluster K8s indisponible).

## Sécurité — compte dédié

Le compte `synthetic-monitor@faso.gov.bf` est :

- Créé via script admin `scripts/seed-synthetic-user.sh` (hors-dépôt).
- Non couplé à un citoyen réel (email `@faso.gov.bf`, domaine opéré).
- Données produites purgées nightly par le cron admin
  (`poulets-order` et `etat-civil`).
- Password rotationné tous les 90 jours via Vault
  (`vault kv put faso/synthetic-monitoring/user …`).

Le fixture `synthetic-user.ts` refuse tout email hors domaine FASO afin
d'éviter un usage accidentel d'un compte citoyen.
