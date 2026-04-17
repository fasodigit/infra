# xds-controller — Canary / Progressive Rollout

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## État

**Livré :** proto `canary/v1/canary.proto` — schéma complet CanaryService (6 RPC), Stage enum (1/10/50 pct, promoted, rolled_back, paused), SloCompliance.

**TODO (implémentation Rust, Vague 2) :**
- `crates/xds-server/src/canary.rs` — `CanaryOrchestrator`, state machine 30 s tick, Prometheus query polling, auto-advance / rollback
- `crates/xds-server/src/canary_config.rs` — YAML parser
- `crates/xds-server/src/services/canary_grpc.rs` — tonic service impl
- `crates/xds-server/src/prometheus_client.rs` — async HTTP query `rate(...)`, `histogram_quantile(0.99, ...)`
- `crates/xds-cli/src/commands/canary.rs` — `xdsctl canary start|pause|abort|promote|status`
- Integration tests `tests/canary_rollout.rs` avec mock Prometheus

## State machine cible

```
Stage1Pct ──(1h OK)──► Stage10Pct ──(1h OK)──► Stage50Pct ──(1h OK)──► Promoted(100%)
    │                       │                       │
    └───────(SLO breach 3 min)──────────────────────┴──► RolledBack(0%)

Any → Paused (via PauseCanary RPC) → Resume replays from paused stage
Any → Aborted (via AbortCanary RPC) → traffic returns to baseline
```

## Critères SLO par défaut

- `error_rate_max` : baseline + 0.5 %
- `latency_p99_max_ms` : baseline × 1.2
- Mesure toutes 30 s, rollback si 3 mesures consécutives out-of-budget

## Intégration xDS

- **CDS** : publier le cluster `<service>-canary` en parallèle de `<service>-stable`
- **RDS** : update `route.route.weighted_clusters` avec poids `stable=99, canary=1` → `90/10` → `50/50` → `0/100`
- Push delta ADS à chaque transition, stream existant (`ads.rs:1-100`)

## Exemple config YAML (futur `canary.yaml`)

```yaml
canaries:
  - service: poulets-api
    baseline_cluster: poulets-api-stable
    canary_cluster: poulets-api-canary
    stages: [1, 10, 50, 100]
    min_stage_duration_secs: 3600
    slo:
      error_rate_max: 0.005
      latency_p99_max_ms: 50
      prometheus_endpoint: "http://prometheus:9090"
```

## Dépendances Rust à ajouter

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
# prost-build ajouté à build.rs pour canary.v1
```

## Liens

- Proto : `proto/canary/v1/canary.proto` (livré)
- ADS existant : `crates/xds-server/src/services/ads.rs`
- Plan global : `/home/lyna/.claude/plans/woolly-growing-galaxy.md` (axe #15)
