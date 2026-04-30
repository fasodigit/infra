<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# FasoArmageddonLatencyHigh — Runbook

**Severity** : Critical
**Alert** : `FasoArmageddonLatencyHigh`
**Oncall** : See `/observability/oncall.yml`
**SLA** : diagnostic + mitigation < 10 min
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Alert `FasoArmageddonLatencyHigh` firing (P99 latency > 100ms)
- Connection queue growing (`armageddon_connections_queued` increasing)
- Upstream health checks failing in ARMAGEDDON admin dashboard
- HTTP 502/503 responses increasing at the edge
- Circuit breakers tripping on backend upstreams
- CPU/memory usage elevated on ARMAGEDDON instances

## Impact

- **User-facing** : Slow page loads, timeouts on all platform routes (auth, catalog, orders)
- **Business** : User abandonment increases; all API traffic routed through ARMAGEDDON is affected
- **SLO** : `gateway-latency` and `gateway-availability` SLO burn rate critical
- **Scope** : ARMAGEDDON is the single entry point — ALL services are impacted

## Diagnosis

### Step 1: Check ARMAGEDDON stats

```bash
# Admin API (loopback only, port 9903)
curl -s http://localhost:9903/admin/stats | jq .

# Key metrics
curl -s http://localhost:9903/admin/stats | jq '{
  active_connections: .active_connections,
  queued_connections: .queued_connections,
  total_requests: .total_requests,
  error_rate: .error_count,
  p99_latency_ms: .latency_p99_ms
}'
```

### Step 2: Check upstream health

```bash
curl -s http://localhost:9903/admin/clusters | jq '.[] | {name: .name, healthy: .healthy_count, unhealthy: .unhealthy_count, circuit_breaker: .circuit_breaker_state}'
```

If any upstream shows `unhealthy > 0` or `circuit_breaker_state: "open"`, the issue is likely upstream, not ARMAGEDDON itself.

### Step 3: Check circuit breaker state

```bash
curl -s http://localhost:9903/admin/clusters | jq '.[] | select(.circuit_breaker_state != "closed") | {name: .name, state: .circuit_breaker_state, last_transition: .cb_last_transition}'
```

### Step 4: Check rate limiting

```bash
curl -s http://localhost:9903/admin/stats | jq '.rate_limit_active, .rate_limited_requests'
# Check SENTINEL mode
curl -s http://localhost:9903/admin/server_info | jq '.sentinel_mode'
```

### Step 5: Check resource usage

```bash
# CPU and memory
podman stats --no-stream faso-armageddon
# Kubernetes
kubectl top pod -l app=armageddon -n faso

# File descriptors (connection limit)
podman exec faso-armageddon cat /proc/1/limits | grep "Max open files"
podman exec faso-armageddon ls /proc/1/fd | wc -l
```

If FD count approaches limit, ARMAGEDDON cannot accept new connections.

### Step 6: Check for DDoS indicators

```bash
# Request rate by source IP (from ARMAGEDDON access logs)
podman logs --tail 10000 faso-armageddon | jq -r '.client_ip' | sort | uniq -c | sort -rn | head -20

# Unusual user-agents
podman logs --tail 10000 faso-armageddon | jq -r '.user_agent' | sort | uniq -c | sort -rn | head -10
```

## Remediation

### Quick Fix (< 5 min)

1. **Scale replicas** :
   ```bash
   kubectl scale deploy/armageddon -n faso --replicas=3
   ```

2. **Adjust rate limits** (reduce per-client limit to protect backends):
   ```bash
   curl -X POST http://localhost:9903/admin/runtime_modify \
     -d 'rate_limit.requests_per_second=50'
   ```

3. **Circuit-break unhealthy upstreams** :
   ```bash
   # Force circuit breaker on a specific upstream
   curl -X POST http://localhost:9903/admin/clusters/poulets-api/circuit_breaker \
     -d '{"state": "force_open"}'
   ```

4. **Enable response caching** (if not already enabled):
   ```bash
   curl -X POST http://localhost:9903/admin/runtime_modify \
     -d 'response_cache.enabled=true' \
     -d 'response_cache.ttl_seconds=30'
   ```

### If DDoS suspected: Enable SENTINEL high-paranoia mode

```bash
curl -X POST http://localhost:9903/admin/runtime_modify \
  -d 'sentinel.mode=high_paranoia' \
  -d 'sentinel.challenge_threshold=10'
```

SENTINEL high-paranoia mode:
- Enables JavaScript proof-of-work challenge for suspicious IPs
- Drops connections exceeding 100 req/s per IP
- Activates GeoIP filtering (if configured)
- Enables request body size limits (1MB default)

### Root Cause Fix

- **Upstream slow** : Fix the slow upstream service (see respective runbook)
- **Connection leak** : Check `keepalive_timeout` and `idle_connection_timeout` settings
- **CPU saturation** : Profile with `perf top` or Tempo traces; optimize hot path
- **Memory pressure** : Check for large request/response bodies; enable streaming

## Escalation

| Time | Action |
|------|--------|
| 0 min | Oncall acknowledges, checks stats and upstream health |
| 5 min | If DDoS suspected, enable SENTINEL high-paranoia immediately |
| 10 min | If upstream issue, escalate to responsible service team |
| 15 min | If not resolved, escalate to SRE lead |
| 30 min | If not resolved, page engineering manager |
| 60 min | Incident commander activated; public status page updated |

## Prevention

- Auto-scaling based on connection count and latency percentiles
- Rate limiting per client IP with progressive enforcement
- Regular load testing with k6 to validate capacity thresholds
- Circuit breaker tuning: review thresholds quarterly
- DDoS playbook rehearsal quarterly
- Monitor file descriptor usage with alert at 80% of limit

## Related Alerts

- `FasoAuthMsDown` — unhealthy upstream causes ARMAGEDDON 503s
- `FasoPouletsApiDown` — unhealthy upstream causes ARMAGEDDON 503s
- `FasoSvidExpiryCritical` — mTLS failure to upstreams looks like upstream failure
- `KayaAvailabilityBurn` — KAYA down degrades ARMAGEDDON response caching
