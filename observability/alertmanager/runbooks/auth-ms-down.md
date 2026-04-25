<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# FasoAuthMsDown — Runbook

**Severity** : Critical
**Alert** : `FasoAuthMsDown`
**Oncall** : See `/observability/oncall.yml`
**SLA** : diagnostic + mitigation < 10 min
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Alert `FasoAuthMsDown` firing in Alertmanager
- HTTP 5xx on `/api/auth/*` endpoints
- Kratos session validation failing (downstream 401/503)
- gRPC port 8702 unreachable from mesh clients
- Users unable to log in, register, or refresh tokens
- ARMAGEDDON health checks report `auth-ms` upstream as unhealthy

## Impact

- **User-facing** : All authentication flows broken (login, registration, password reset, OTP)
- **Business** : Platform fully unusable for unauthenticated operations; existing sessions may degrade if KAYA cache expires
- **SLO** : `auth-availability` SLO burn rate spike (critical page tier)
- **Downstream** : `poulets-api`, `notifier-ms`, `bff-nextjs` all depend on auth-ms for token validation

## Diagnosis

### Step 1: Check pod / container status

```bash
# Kubernetes
kubectl get pods -l app=auth-ms -n faso
kubectl describe pod -l app=auth-ms -n faso | tail -40

# Local dev (podman)
podman ps --filter name=faso-auth-ms
podman logs --tail 200 faso-auth-ms
```

Expected: Pod should be `Running` with `1/1` ready. If `CrashLoopBackOff` or `OOMKilled`, check logs for root cause.

### Step 2: Check Kratos health

```bash
curl -s http://localhost:4434/health/ready | jq .
curl -s http://localhost:4434/health/alive | jq .
```

Expected: `{"status":"ok"}`. If Kratos is down, auth-ms cannot validate sessions.

### Step 3: Check database connectivity

```bash
pg_isready -h localhost -p 5432 -U auth_ms
# or from inside the container
podman exec faso-auth-ms curl -s http://localhost:9002/actuator/health/db | jq .
```

Expected: `accepting connections`. Check HikariCP pool exhaustion:

```bash
podman exec faso-auth-ms curl -s http://localhost:9002/actuator/metrics/hikaricp.connections.active | jq .
podman exec faso-auth-ms curl -s http://localhost:9002/actuator/metrics/hikaricp.connections.pending | jq .
```

If `pending > 0` for extended period, pool is exhausted.

### Step 4: Check KAYA (session cache)

```bash
podman exec faso-kaya kaya-cli ping
# Check session key count
podman exec faso-kaya kaya-cli DBSIZE
podman exec faso-kaya kaya-cli INFO memory
```

If KAYA is down, auth-ms falls back to DB but with higher latency.

### Step 5: Check gRPC port 8702

```bash
# Verify port is listening
ss -tlnp | grep 8702
# gRPC health check (if grpcurl available)
grpcurl -plaintext localhost:8702 grpc.health.v1.Health/Check
```

### Step 6: Check Vault secrets

```bash
vault kv get faso/auth-ms/db
vault kv get faso/auth-ms/kratos
```

If secrets are missing or expired, auth-ms cannot start.

## Remediation

### Quick Fix (< 5 min)

```bash
# Restart the pod
kubectl rollout restart deploy/auth-ms -n faso

# Local dev
podman restart faso-auth-ms
```

If the pod fails to start after restart, check for:

1. **DB migration failure** :
   ```bash
   podman exec faso-auth-ms curl -s http://localhost:9002/actuator/flyway | jq '.contexts[].flywayBeans[].migrations[-1]'
   ```
   If last migration failed, manually repair:
   ```bash
   podman exec faso-auth-ms java -jar app.jar flyway repair
   ```

2. **HikariCP pool exhaustion** :
   ```bash
   # Check for long-running queries
   psql -h localhost -p 5432 -U postgres -c "SELECT pid, now() - pg_stat_activity.query_start AS duration, query FROM pg_stat_activity WHERE state != 'idle' AND datname = 'auth_ms' ORDER BY duration DESC LIMIT 10;"
   ```

3. **Vault sealed** : See `vault-sealed.md` runbook.

### Root Cause Fix

- If OOMKilled: increase memory limits in deployment manifest
- If HikariCP exhausted: increase pool size or fix slow queries
- If Kratos migration mismatch: run Kratos migration, then restart auth-ms
- If SVID expired: restart SPIRE agent (see `certificate-rotation-failed.md`)

### Fallback: ARMAGEDDON degraded mode

If auth-ms cannot be restored quickly, ARMAGEDDON can serve cached JWT validation (read-only, degraded mode):

```bash
# Enable degraded auth mode in ARMAGEDDON
curl -X POST http://localhost:9903/admin/runtime_modify \
  -d 'auth.degraded_mode=true'
```

**Warning**: In degraded mode, new logins are blocked; only existing valid JWTs work. Session revocation is not enforced.

## Escalation

| Time | Action |
|------|--------|
| 0 min | Oncall acknowledges, starts diagnosis |
| 10 min | If DB issue, escalate to DBA oncall |
| 15 min | If not resolved, escalate to SRE lead |
| 30 min | If not resolved, page engineering manager + enable degraded mode |
| 60 min | Incident commander activated, public status page updated |

## Prevention

- Monitor HikariCP pool usage with alert at 80% utilization
- Ensure Flyway migrations are tested in staging before prod deploy
- Set up synthetic login probe (every 30s) to catch issues before users
- Keep Kratos version pinned and tested with auth-ms release

## Related Alerts

- `KayaAvailabilityBurn` — KAYA down degrades session cache
- `FasoVaultSealed` — secrets unavailable blocks auth-ms startup
- `FasoSvidExpiryCritical` — mTLS failure blocks gRPC mesh
- `FasoPouletsApiDown` — downstream impact from auth-ms outage
- `FasoArmageddonLatencyHigh` — may fire if auth-ms latency spikes
