# Chaos Engineering — FASO DIGITALISATION

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- sovereignty=true -->

Framework de chaos engineering basé sur **Chaos Mesh** pour valider la résilience des microservices FASO DIGITALISATION en conditions de panne simulée.

## Installation

Voir [`install/README.md`](install/README.md) pour le déploiement Helm complet.

## Exécution manuelle d'une expérience

```bash
# Appliquer une expérience immédiatement (sans schedule)
kubectl apply -f experiments/pod-kill-kaya.yaml -n faso

# Suivre l'état en temps réel
kubectl get podchaos -n faso -w

# Vérifier les events injectés
kubectl describe podchaos pod-kill-kaya-nightly -n faso

# Supprimer (arrêt de l'expérience)
kubectl delete -f experiments/pod-kill-kaya.yaml -n faso
```

### Exécution one-shot (sans schedule récurrent)

Pour tester manuellement sans activer le cron, utiliser `kubectl apply` suivi d'un patch :

```bash
# Suspendre le schedule après la première exécution
kubectl patch schedule pod-kill-kaya-nightly -n faso \
  --type merge -p '{"spec":{"concurrencyPolicy":"Forbid"}}'
```

### Lister toutes les expériences actives

```bash
kubectl get schedules,podchaos,networkchaos,timechaos,iochaos,stresschaos -n faso
```

---

## Matrice Expériences × Services

| Fichier | Type Chaos | Service(s) cible | Schedule | Durée | Vérification |
|---|---|---|---|---|---|
| `pod-kill-kaya.yaml` | PodChaos `pod-kill` | KAYA | `0 2 * * *` (nightly) | 30s | Redémarrage replica, quorum Raft maintenu |
| `network-partition.yaml` | NetworkChaos `partition` | KAYA ↔ ARMAGEDDON | `0 3 * * *` (nightly) | 30s | Circuit breaker ARMAGEDDON ouvert, retries post-recovery |
| `clock-skew.yaml` | TimeChaos `+5m` | auth-ms | `0 4 * * 0` (hebdo dimanche) | 60s | JWT non rejetés (clock leeway), pas de 401 spurieux |
| `io-delay-postgres.yaml` | IOChaos `latency 200ms` | auth-ms / PostgreSQL | `0 2 * * 2` (mardi) | 60s | P99 latency < 500ms, connection pool stable |
| `memory-stress-armageddon.yaml` | StressChaos RAM 80% | ARMAGEDDON | `0 3 * * 3` (mercredi) | 60s | Pas d'OOMKilled, backpressure activée |
| `dns-failure.yaml` | NetworkChaos `dns error` | poulets-api | `0 4 * * 4` (jeudi) | 60s | Fallback DNS cache, pas de 5xx propagés clients |

---

## SLO Cibles pendant le chaos

Ces objectifs définissent le comportement attendu pendant **et après** l'injection de faute.

### P99 Latency

| Service | SLO P99 (baseline) | SLO P99 (sous chaos) | Mesure |
|---|---|---|---|
| KAYA (gRPC reads) | < 5ms | < 50ms | Prometheus `grpc_server_handling_seconds_bucket` |
| ARMAGEDDON (API) | < 100ms | < 800ms | `http_request_duration_seconds_bucket` |
| auth-ms (REST) | < 80ms | < 500ms | `http_request_duration_seconds_bucket` |
| poulets-api (REST) | < 120ms | < 600ms | `http_request_duration_seconds_bucket` |

### Error Rate

| Service | SLO Error Rate (baseline) | SLO Error Rate (sous chaos) |
|---|---|---|
| KAYA | < 0.01% | < 1% |
| ARMAGEDDON | < 0.1% | < 5% (circuit breaker attendu) |
| auth-ms | < 0.1% | < 2% |
| poulets-api | < 0.1% | < 3% (retry + DNS cache) |

### Recovery Time Objective (RTO)

| Expérience | RTO attendu |
|---|---|
| pod-kill KAYA | < 30s (redémarrage + réintégration Raft) |
| network-partition | < 10s après rétablissement (retries automatiques) |
| clock-skew auth-ms | 0s (tolérance JWT leeway, service transparent) |
| io-delay postgres | 0 panne, dégradation contrôlée P99 |
| memory-stress | 0 OOMKill, backpressure maintenue |
| dns-failure | < 5s (cache TTL + retry policy) |

---

## Rapport automatique

Le workflow `.github/workflows/chaos-report.yml` :
- S'exécute chaque nuit à **05h00 UTC** (après toutes les expériences)
- Collecte les résultats via `kubectl get` + events Kubernetes
- Vérifie les pods redémarrés et OOMKills
- Tente de fetcher un screenshot Grafana (dashboard `faso-slo`)
- Crée une issue GitHub `Chaos Nightly Report YYYY-MM-DD`

### Secrets requis (GitHub Actions)

| Secret | Description |
|---|---|
| `KUBECONFIG_FASO` | kubeconfig base64 du cluster FASO |
| `GRAFANA_URL` | URL Grafana (ex: `https://grafana.faso.internal`) |
| `GRAFANA_TOKEN` | Service Account token Grafana (viewer) |

---

## Labels et gouvernance

Chaque expérience porte les labels :

```yaml
labels:
  sovereignty: "true"
  team: faso-sre
```

Toutes les ressources sont sous licence **AGPL-3.0-or-later**.
