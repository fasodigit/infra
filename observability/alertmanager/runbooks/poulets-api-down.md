<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# FasoPouletsApiDown — Runbook

**Severity** : Critical
**Alert** : `FasoPouletsApiDown`
**Oncall** : See `/observability/oncall.yml`
**SLA** : diagnostic + mitigation < 10 min
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Alert `FasoPouletsApiDown` firing in Alertmanager
- GraphQL endpoint returning errors or timeouts
- Order placement fails (HTTP 500/503 on `/api/orders`)
- BFF reports upstream errors for poulets-api (port 8901)
- Actuator endpoint `http://localhost:9001/actuator/health` returning DOWN
- Redpanda consumer lag increasing on `poulets.*` topics

## Impact

- **User-facing** : Catalog browsing degraded, order placement fully blocked, payment flow interrupted
- **Business** : Revenue loss — no new orders can be placed; existing orders in processing stall
- **SLO** : `poulets-availability` SLO burn rate critical
- **Downstream** : notifier-ms stops processing order events; BFF returns error pages

## Diagnosis

### Step 1: Check pod / container status

```bash
# Kubernetes
kubectl get pods -l app=poulets-api -n faso
kubectl describe pod -l app=poulets-api -n faso | tail -40

# Local dev (podman)
podman ps --filter name=faso-poulets-api
podman logs --tail 200 faso-poulets-api
```

### Step 2: Check database connection pool

```bash
# HikariCP metrics via actuator
podman exec faso-poulets-api curl -s http://localhost:9001/actuator/metrics/hikaricp.connections.active | jq .
podman exec faso-poulets-api curl -s http://localhost:9001/actuator/metrics/hikaricp.connections.pending | jq .
podman exec faso-poulets-api curl -s http://localhost:9001/actuator/metrics/hikaricp.connections.timeout.total | jq .
```

Check for DB locks:

```bash
psql -h localhost -p 5432 -U postgres -d poulets <<'SQL'
SELECT pid, now() - pg_stat_activity.query_start AS duration, state, query
FROM pg_stat_activity
WHERE datname = 'poulets'
  AND state != 'idle'
ORDER BY duration DESC
LIMIT 20;
SQL
```

### Step 3: Check Flyway migration status

```bash
podman exec faso-poulets-api curl -s http://localhost:9001/actuator/flyway | jq '.contexts[].flywayBeans[].migrations[-3:]'
```

If last migration is `FAILED`, the pod may not start correctly.

### Step 4: Check KAYA cache

```bash
podman exec faso-kaya kaya-cli ping
podman exec faso-kaya kaya-cli KEYS "poulets:catalog:*" | head -5
podman exec faso-kaya kaya-cli INFO memory
```

### Step 5: Check Redpanda consumer health

```bash
rpk group describe poulets-consumer-group
# Check for lag > 0 on any partition
rpk topic consume poulets.orders.v1 --num 1 --offset end
```

### Step 6: Check auth-ms dependency

```bash
curl -s http://localhost:8801/actuator/health | jq .status
curl -s http://localhost:9002/actuator/health | jq .status
```

If auth-ms is down, poulets-api may fail on token validation.

## Remediation

### Quick Fix (< 5 min)

```bash
# Restart
kubectl rollout restart deploy/poulets-api -n faso

# Local dev
podman restart faso-poulets-api
```

If restart fails:

1. **Kill stuck DB connections** :
   ```sql
   SELECT pg_terminate_backend(pid)
   FROM pg_stat_activity
   WHERE datname = 'poulets'
     AND state = 'idle in transaction'
     AND query_start < NOW() - INTERVAL '5 minutes';
   ```

2. **Fix Flyway migration** :
   ```bash
   podman exec faso-poulets-api java -jar app.jar flyway repair
   podman restart faso-poulets-api
   ```

3. **Check Vault secrets** :
   ```bash
   vault kv get faso/poulets-api/db
   ```

### Root Cause Fix

- If OOMKilled: increase memory limits, analyze heap dump
- If connection pool exhausted: tune HikariCP `maximumPoolSize`, fix N+1 queries
- If Redpanda consumer stuck: reset consumer offset or restart consumer threads
- If Flyway failure: fix migration SQL, repair, restart

### Fallback: Read-only mode via KAYA cache

If poulets-api is fully down, ARMAGEDDON can serve cached catalog data from KAYA:

```bash
# Enable read-only fallback in ARMAGEDDON
curl -X POST http://localhost:9903/admin/runtime_modify \
  -d 'poulets.fallback_mode=cache_readonly'
```

**Limitations in fallback** : Catalog is browsable (cached data), prices may be stale, orders are queued in KAYA and replayed when poulets-api recovers. No new registrations.

## Escalation

| Time | Action |
|------|--------|
| 0 min | Oncall acknowledges, starts diagnosis |
| 10 min | If DB issue, escalate to DBA oncall |
| 15 min | If not resolved, escalate to SRE lead |
| 30 min | If not resolved, page engineering manager + activate read-only fallback |
| 60 min | Incident commander activated |

## Prevention

- Synthetic order probe every 60s (staging environment)
- Monitor HikariCP pool usage, alert at 80%
- Monitor Redpanda consumer lag, alert if lag > 1000
- Pre-deploy Flyway dry-run in CI pipeline
- Load test with k6 before major releases

## Related Alerts

- `FasoAuthMsDown` — auth-ms is a hard dependency for token validation
- `FasoKafkaConsumerLagCritical` — Redpanda consumer lag may indicate poulets-api processing issues
- `KayaAvailabilityBurn` — KAYA down removes cache fallback
- `FasoPostgresReplicationLag` — read replica issues may cause stale reads
- `FasoArmageddonLatencyHigh` — upstream latency from poulets-api propagates
