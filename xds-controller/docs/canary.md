<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 FASO DIGITALISATION -->

# Canary — Progressive Rollout via xDS

## Architecture decisionnelle

```
                           ┌─────────────────────────────────────────────┐
                           │           CanaryOrchestrator (30 s tick)    │
                           │                                             │
  xdsctl canary start ───► │  start() → Stage1Pct (1%)                  │
                           │     │                                       │
                           │  tick_one():                                │
                           │     1. query Prometheus (parallel joins)    │
                           │        - error_rate = rate(5xx)[5m]        │
                           │        - latency_p99 = histogram_quantile  │
                           │     2. SLO gate:                            │
                           │        ok  → reset breach counter          │
                           │        bad → consecutive_breaches++        │
                           │             breaches >= 3 → rollback       │
                           │     3. stage_elapsed >= min_stage_duration  │
                           │        AND last tick ok → advance stage    │
                           │                                             │
                           │  Stage1Pct → Stage10Pct → Stage50Pct      │
                           │           → Promoted (100%)                │
                           └──────────────┬──────────────────────────────┘
                                          │ apply_weights()
                                          ▼
                                   ConfigStore (RDS)
                                   route.weighted_clusters:
                                     - stable: (100 - canary)%
                                     - canary: N%
                                          │
                                          │ watch channel notify
                                          ▼
                               ADS push → ARMAGEDDON instances
```

### Gate SLO

Chaque tick (30 s) evaluate deux metriques PromQL sur le subset canary :

| Metrique | Requete PromQL | Seuil defaut |
|---|---|---|
| Taux d'erreur | `sum(rate(http_requests_total{cluster="<svc>-canary",status=~"5.."}[5m])) / sum(rate(...))` | 0.5% |
| Latence p99 | `histogram_quantile(0.99, sum(rate(http_request_duration_seconds_bucket{cluster="<svc>-canary"}[5m])) by (le)) * 1000` | 50 ms |

**Regle de blocage** : si l'une des deux metriques depasse son seuil, `consecutive_breaches` s'incremente. Quand `consecutive_breaches >= 3` (soit 90 s de SLO breach), le rollback automatique est declenche : poids canary → 0%, etat → `RolledBack`.

**Avancement** : progression vers l'etape suivante uniquement si :
- `stage_elapsed >= min_stage_duration` (defaut 1 h)
- Le dernier tick etait dans le budget SLO

**Prometheus injoignable** : le tick est saute (SLO inconnu != SLO breach). Le compteur de breaches ne s'incremente pas.

### Progression des poids xDS

```
Stage1Pct   : stable=99%  canary=1%   (WeightedCluster)
Stage10Pct  : stable=90%  canary=10%
Stage50Pct  : stable=50%  canary=50%
Promoted    : canary=100% (cluster unique, stable retire de la route)
RolledBack  : stable=100% (cluster unique, canary retire de la route)
```

Chaque changement de poids ecrit dans `ConfigStore.set_route()` qui notifie via `watch::Sender` tous les streams ADS actifs — push delta immediat vers ARMAGEDDON.

---

## Integration avec Argo Rollouts (post-hook)

Le `CanaryService` gRPC peut etre utilise comme **Metric Provider** personnalise pour Argo Rollouts via un webhook post-step :

```yaml
# rollout.yaml (extrait)
analysis:
  templates:
    - templateName: xds-canary-gate
  args:
    - name: canary-id
      value: "{{inputs.parameters.canary-id}}"
---
# analysistemplate.yaml
spec:
  metrics:
    - name: xds-slo-gate
      provider:
        web:
          url: "http://xds-controller:18000/faso.canary.v1.CanaryService/GetCanaryStatus"
          jsonPath: "{$.slo_compliance.within_budget}"
      successCondition: result == true
```

L'API gRPC expose `GetCanaryStatus` qui retourne `slo_compliance.within_budget` consultable par tout metric provider HTTP.

---

## Runbook operateur

### Demarrer un canary

```bash
xdsctl canary start \
  --service poulets-api \
  --image-tag v1.3.0 \
  --prometheus http://prometheus:9090 \
  --error-rate-max 0.005 \
  --latency-p99-max 50 \
  --min-stage-secs 3600
# Output: canary_id = 4f3a1b2c-...
```

### Surveiller la progression

```bash
# Snapshot unique
xdsctl canary status --canary-id 4f3a1b2c-...

# Liste tous les canaries du service
xdsctl canary list --service poulets-api
```

Champ `stage` affiche : `1pct` → `10pct` → `50pct` → `promoted` | `rolled_back` | `paused`.

### Pauser (figer les poids, stopper l'avancement)

```bash
xdsctl canary pause --canary-id 4f3a1b2c-...
# Les poids xDS restent a leur niveau actuel; le tick ne progresse plus.
```

### Reprendre (promotion manuelle)

```bash
# Force-promouvoir a 100% independamment du SLO
xdsctl canary promote --canary-id 4f3a1b2c-...
```

### Annuler (rollback immediat)

```bash
xdsctl canary abort \
  --canary-id 4f3a1b2c-... \
  --reason "regression CPU observee en production"
# Poids canary → 0% instantanement; etat → rolled_back
```

---

## Dashboard Grafana

Importer le dashboard `xds-canary-overview` (panel JSON disponible dans
`INFRA/observability/grafana/dashboards/xds-canary.json`).

Panels recommandes :

| Panel | Metrique | Alerte |
|---|---|---|
| Canary weight | `xds_canary_weight_pct{service="poulets-api"}` | — |
| Error rate canary vs stable | `rate(http_requests_total{status=~"5.."}[5m])` par cluster | > seuil SLO |
| p99 latency canary | `histogram_quantile(0.99, ...)` | > seuil SLO |
| Consecutive breaches | `xds_canary_consecutive_breaches` | >= 2 (warn), >= 3 (crit) |
| Stage transitions | Logs tracing `canary advanced to next stage` | — |

Alerte Prometheus recommandee :

```yaml
- alert: CanaryAutoRollback
  expr: xds_canary_stage{stage="rolled_back"} == 1
  for: 0m
  labels:
    severity: warning
  annotations:
    summary: "Canary {{ $labels.service }} rolled back automatically"
    description: "SLO breach detected after 3 consecutive ticks (90s)"
```

---

## Modes de defaillance

| Scenario | Comportement |
|---|---|
| Prometheus injoignable | Tick saute (SLO inconnu); pas de breach; log `WARN` |
| Cluster canary absent du store | Cree automatiquement au `start()` en copiant la config stable |
| Mutation concurrente (deux RPCs simultanes) | `DashMap` atomique; aucun lock tenu a travers `.await` |
| Perte du xDS Controller | ARMAGEDDON conserve le dernier snapshot recu; pas de blackout |
| `apply_weights` echoue en ecriture store | `ERROR` trace; poids xDS non mis a jour; prochain tick retente |

---

## Metriques Prometheus exposees par xds-controller

| Metrique | Type | Labels | Description |
|---|---|---|---|
| `xds_canary_stage` | Gauge | `service`, `stage` | Stage actuel (1=actif) |
| `xds_canary_weight_pct` | Gauge | `service` | Poids canary courant |
| `xds_canary_consecutive_breaches` | Gauge | `service` | Compteur de breaches consecutifs |
| `xds_canary_rollback_total` | Counter | `service`, `reason` | Rollbacks auto/manuels |
| `xds_canary_promotion_total` | Counter | `service` | Promotions reussies |
