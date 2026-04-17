# Runbook — KAYA Down

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

**Alert** : `KayaAvailabilityBurn` ou `ServiceDown{service="kaya"}`

## Diagnostic rapide (< 2 min)

```bash
# 1. Est-ce que le process tourne ?
podman ps --filter name=faso-kaya
# 2. Logs récents
podman logs --tail 100 faso-kaya
# 3. Ping RESP3
podman exec faso-kaya kaya-cli ping
# Attendu : PONG
# 4. Metrics Prometheus
curl -s http://localhost:9100/metrics | grep kaya_
```

## Mitigations

| Symptôme | Action |
|----------|--------|
| **OOMKilled** | Augmenter `KAYA_MAX_MEMORY` ou baisser `num_shards` |
| **WAL fsync saturé** | Switcher `fsync: everysec` si prod prod `always` |
| **Port 6380 bind refused** | Vérifier conflit avec Redis orphelin ; `ss -tlnp | grep 6380` |
| **Eviction 100 %** | LFU policy saturée → scale horizontal (Raft cluster) |
| **Shard panic** | Logs JSON `panic=` → restart avec `KAYA_LOG_LEVEL=debug` + capture `--logs` |

## Escalation

1. 5 min : oncall L1 tente restart `podman restart faso-kaya`
2. 10 min : si replica in cluster → failover manuel, vérifier quorum Raft
3. 15 min : escalade lead plateforme (trigger pagerduty)

## Post-récupération

- [ ] Vérifier WAL integrity : `cargo run -p kaya-cli -- persistence inspect --data-dir /var/lib/kaya`
- [ ] Diff avec autre replica si cluster Raft
- [ ] Update postmortem issue avec timeline
