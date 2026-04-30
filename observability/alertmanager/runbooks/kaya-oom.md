<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# FasoKayaOOM — Runbook

**Severity** : Critical
**Alert** : `FasoKayaOOM`
**Oncall** : See `/observability/oncall.yml`
**SLA** : diagnostic + mitigation < 10 min
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Alert `FasoKayaOOM` firing in Alertmanager
- KAYA pod/container status: `OOMKilled` or restarting
- Session cache miss rate spiking across all services
- auth-ms latency increase (falling back to DB for session lookups)
- `kaya_memory_used_bytes` approaching `kaya_memory_max_bytes` in Grafana
- Client-side errors: `KAYA connection refused` or `KAYA timeout`

## Impact

- **User-facing** : Increased latency on all authenticated requests (cache miss penalty); possible session drops if KAYA data lost
- **Business** : Platform performance degraded; catalog/pricing data stale if cache invalidated
- **SLO** : `kaya-availability` SLO burn, plus cascading latency SLO burn on auth-ms and poulets-api
- **Data risk** : If WAL is not caught up, recent writes may be lost on restart

## Diagnosis

### Step 1: Check container status and reason

```bash
# Check if OOMKilled
podman inspect faso-kaya | jq '.[0].State'

# Kubernetes
kubectl get pods -l app=kaya -n faso
kubectl describe pod -l app=kaya -n faso | grep -A5 "Last State\|OOMKilled\|Reason"
```

### Step 2: Check memory usage (if still running)

```bash
podman exec faso-kaya kaya-cli INFO memory
```

Key metrics to check:
- `used_memory` vs `maxmemory` — how close to limit
- `used_memory_rss` — actual RSS (OS perspective)
- `mem_fragmentation_ratio` — if > 1.5, significant fragmentation
- `used_memory_lua` / `used_memory_scripts` — Rhai/WASM script memory

### Step 3: Check key count and distribution

```bash
# Total keys
podman exec faso-kaya kaya-cli DBSIZE

# Key namespace distribution
podman exec faso-kaya kaya-cli KEYS "*" | sed 's/:.*//' | sort | uniq -c | sort -rn | head -20

# Largest keys (memory analysis)
podman exec faso-kaya kaya-cli --bigkeys
```

### Step 4: Check eviction stats

```bash
podman exec faso-kaya kaya-cli INFO stats | grep evicted
podman exec faso-kaya kaya-cli CONFIG GET maxmemory-policy
```

If `maxmemory-policy` is `noeviction`, KAYA will OOM instead of evicting.

### Step 5: Check WAL size

```bash
# WAL file size on disk
podman exec faso-kaya ls -lh /var/lib/kaya/wal/
podman exec faso-kaya du -sh /var/lib/kaya/
```

Large WAL may indicate compaction is behind.

### Step 6: Identify memory growth source

```bash
# Check Prometheus for memory growth over last 24h
# Grafana: dashboard KAYA Overview, panel "Memory Usage"
curl -s "http://localhost:9090/api/v1/query?query=kaya_memory_used_bytes[24h]" | jq '.data.result[0].values[-5:]'
```

## Remediation

### Quick Fix (< 5 min)

1. **Emergency eviction of non-critical namespaces** :
   ```bash
   # Flush non-critical caches (catalog cache is rebuildable)
   podman exec faso-kaya kaya-cli DEL $(podman exec faso-kaya kaya-cli KEYS "poulets:catalog:*")
   podman exec faso-kaya kaya-cli DEL $(podman exec faso-kaya kaya-cli KEYS "poulets:pricing:*")
   ```
   **DO NOT flush session keys** (`auth:session:*`) — these are critical.

2. **Set eviction policy** (if currently `noeviction`):
   ```bash
   podman exec faso-kaya kaya-cli CONFIG SET maxmemory-policy allkeys-lfu
   ```

3. **Increase memory limit** (if headroom available on host):
   ```bash
   # Kubernetes
   kubectl patch deploy kaya -n faso --type='json' \
     -p='[{"op":"replace","path":"/spec/template/spec/containers/0/resources/limits/memory","value":"2Gi"}]'

   # Local podman — restart with higher limit
   podman update --memory=2g faso-kaya
   ```

4. **Restart if OOMKilled** :
   ```bash
   podman restart faso-kaya
   # Monitor WAL replay
   podman logs -f faso-kaya | grep -i "wal\|replay\|recovery"
   ```

### Root Cause Fix

- **Unbounded key growth** : Identify which service is writing keys without TTL
  ```bash
  podman exec faso-kaya kaya-cli TTL "problematic:key:pattern:1"
  ```
  If TTL is `-1` (no expiry), fix the application to set TTLs.

- **Memory fragmentation** : If `mem_fragmentation_ratio > 1.5`:
  ```bash
  podman exec faso-kaya kaya-cli MEMORY DOCTOR
  # Consider restart during maintenance window for defrag
  ```

- **Large values** : If a few keys consume disproportionate memory:
  ```bash
  podman exec faso-kaya kaya-cli MEMORY USAGE "large:key:name"
  ```
  Refactor application to use smaller, more granular keys.

## Escalation

| Time | Action |
|------|--------|
| 0 min | Oncall acknowledges, checks memory usage and eviction policy |
| 5 min | If data loss suspected, alert SRE lead immediately |
| 10 min | If WAL replay fails, escalate to KAYA core team |
| 15 min | If not resolved, page engineering manager |
| 30 min | If cluster mode — check Raft quorum, failover to replica |

## Prevention

- Set `maxmemory-policy` to `allkeys-lfu` (never `noeviction` in production)
- Alert on memory usage at 70% (`kaya_memory_used_bytes / kaya_memory_max_bytes > 0.7`)
- Enforce TTLs on all application keys (audit quarterly)
- Monitor key growth rate: alert if daily key count increase > 20%
- WAL compaction cron: ensure background compaction runs every 6h
- Capacity planning: review memory projections monthly

## Post-Recovery

- [ ] Verify WAL integrity: `cargo run -p kaya-cli -- persistence inspect --data-dir /var/lib/kaya`
- [ ] Verify session data consistency with auth-ms
- [ ] Check Raft replica sync (if cluster mode)
- [ ] Update postmortem with root cause and key growth analysis

## Related Alerts

- `KayaAvailabilityBurn` — KAYA down alert (may fire alongside OOM)
- `FasoAuthMsDown` — auth-ms degrades without KAYA session cache
- `FasoPouletsApiDown` — poulets-api catalog cache miss spike
- `FasoArmageddonLatencyHigh` — latency increase from cache misses
