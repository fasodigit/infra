# FASO DIGITALISATION — Synthetic monitoring

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## Scénarios

| Scenario | Flow | Frequency |
|----------|------|-----------|
| `auth-flow.spec.ts` | Landing → login → dashboard → logout | 5 min |
| `poulets-order.spec.ts` | TODO : login eleveur → stock poulet → commande client → confirmation | 5 min |
| `etat-civil-request.spec.ts` | TODO : demande acte naissance → paiement → téléchargement PDF | 5 min |
| `vouchers-redeem.spec.ts` | TODO : agriculteur redeem voucher | 5 min |
| `escool-enrollment.spec.ts` | TODO : parent inscription élève | 5 min |
| `faso-kalan-lesson.spec.ts` | TODO : étudiant accès cours | 5 min |

## Métriques poussées

- `synthetic_<scenario>_duration_ms{env,region}` — durée totale
- `synthetic_<scenario>_success{env,region}` — 0/1

## Exécution locale

```bash
cd INFRA/synthetic-monitoring/playwright
npm install
npx playwright install chromium
FASO_PROD_URL=http://localhost:4801 \
  SYNTHETIC_USER_EMAIL=test@faso.gov.bf \
  SYNTHETIC_USER_PASSWORD=xxx \
  PROM_PUSHGATEWAY_URL=http://localhost:9091 \
  npx playwright test --headed=false
```

## Secrets requis (GitHub + vault prod)

- `SYNTHETIC_USER_EMAIL` / `SYNTHETIC_USER_PASSWORD` — compte de test dédié
- `PROM_PUSHGATEWAY_URL` — endpoint Pushgateway
- `FASO_PROD_URL` (var) — base URL (staging/prod)

## Alerting

- 3 failures consécutifs → alerte Prometheus → Alertmanager → issue GitHub auto (via postmortem-bot)
- Traces + videos Playwright attachés à l'issue
- Runbook : `INFRA/observability/alertmanager/runbooks/synthetic-failure.md`

## Cleanup data

Le scénario `auth-flow` ne crée pas de données. Les scénarios métier (`poulets-order`, `etat-civil-request`, etc.) doivent utiliser un user dédié dont les données sont purgées nightly via cron admin.

## Roadmap

- Multi-région : ajouter `bf-bobo-sud` après déploiement DC secondaire
- Dashboard Grafana `synthetic-monitoring.json` dans `observability/grafana/dashboards/`
- Intégration avec SLO error budget (#12 Sloth)
