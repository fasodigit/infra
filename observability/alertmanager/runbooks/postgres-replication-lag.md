<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# FasoPostgresReplicationLag — Runbook

**Severity** : Warning (escalates to Critical if lag > 5 min)
**Alert** : `FasoPostgresReplicationLag`
**Oncall** : See `/observability/oncall.yml`
**SLA** : diagnostic < 10 min, mitigation < 30 min
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Alert `FasoPostgresReplicationLag` firing
- Read replicas returning stale data (users see outdated catalog, old order status)
- Grafana panel "PG Replication Lag" showing increasing delta
- Replica WAL receiver falling behind primary WAL sender
- `pg_stat_replication.replay_lag` increasing beyond threshold

## Impact

- **User-facing** : Stale reads on catalog, order status, user profile (read-heavy queries routed to replica)
- **Business** : Inventory inconsistencies; users may place orders on out-of-stock items
- **SLO** : `data-freshness` SLO impacted
- **Risk** : If lag exceeds `max_standby_streaming_delay`, replica may cancel queries or disconnect

## Diagnosis

### Step 1: Check replication status on primary

```bash
psql -h localhost -p 5432 -U postgres <<'SQL'
SELECT
    client_addr,
    state,
    sent_lsn,
    write_lsn,
    flush_lsn,
    replay_lsn,
    pg_wal_lsn_diff(sent_lsn, replay_lsn) AS replay_lag_bytes,
    write_lag,
    flush_lag,
    replay_lag
FROM pg_stat_replication;
SQL
```

Key indicators:
- `replay_lag_bytes > 100MB` — significant lag
- `state != 'streaming'` — replication broken
- `replay_lag > '00:05:00'` — CRITICAL, stop routing reads to replica

### Step 2: Check WAL position difference

```bash
psql -h localhost -p 5432 -U postgres -c "
SELECT
    pg_current_wal_lsn() AS primary_lsn,
    pg_wal_lsn_diff(pg_current_wal_lsn(), confirmed_flush_lsn) AS lag_bytes
FROM pg_replication_slots;
"
```

### Step 3: Check network between primary and replica

```bash
# Latency test
ping -c 5 postgres-replica-0

# Bandwidth test (rough estimate from WAL transfer rate)
podman exec faso-postgres cat /var/log/postgresql/postgresql.log | grep -i "replication\|wal.*sender" | tail -20
```

### Step 4: Check disk I/O on replica

```bash
# I/O stats on replica
podman exec faso-postgres-replica iostat -x 1 5
# or
kubectl exec postgres-replica-0 -n faso -- iostat -x 1 5

# Check for I/O wait
podman exec faso-postgres-replica cat /proc/loadavg
```

If `%iowait > 50%`, disk is the bottleneck for WAL replay.

### Step 5: Check for slow queries on replica

```bash
psql -h postgres-replica -p 5432 -U postgres <<'SQL'
SELECT pid, now() - query_start AS duration, state, left(query, 100)
FROM pg_stat_activity
WHERE state != 'idle'
  AND backend_type = 'client backend'
ORDER BY duration DESC
LIMIT 10;
SQL
```

Long-running queries on replica can block WAL replay (conflict).

### Step 6: Check replication slot

```bash
psql -h localhost -p 5432 -U postgres -c "
SELECT slot_name, active, restart_lsn, confirmed_flush_lsn,
       pg_wal_lsn_diff(pg_current_wal_lsn(), restart_lsn) AS retained_wal_bytes
FROM pg_replication_slots;
"
```

If `active = false` and `retained_wal_bytes` is growing, slot is blocking WAL cleanup.

## Remediation

### Quick Fix (< 5 min)

1. **Stop routing reads to lagging replica** :
   ```bash
   # If lag > 5 min, ARMAGEDDON should stop sending reads to replica
   curl -X POST http://localhost:9903/admin/clusters/postgres-replica/circuit_breaker \
     -d '{"state": "force_open"}'
   ```

2. **Kill long-running queries on replica** (if blocking WAL replay):
   ```sql
   -- On replica
   SELECT pg_terminate_backend(pid)
   FROM pg_stat_activity
   WHERE state != 'idle'
     AND query_start < NOW() - INTERVAL '10 minutes'
     AND backend_type = 'client backend';
   ```

3. **Increase `max_standby_streaming_delay`** (temporary):
   ```sql
   -- On replica
   ALTER SYSTEM SET max_standby_streaming_delay = '5min';
   SELECT pg_reload_conf();
   ```

### Root Cause Fix

- **Network bottleneck** : Check inter-node bandwidth; consider dedicated replication network
- **Disk I/O on replica** : Move WAL to SSD; increase `shared_buffers` on replica; tune `effective_io_concurrency`
- **WAL generation spike** : Large bulk operations on primary should be batched; consider `wal_compression = on`
- **Stale replication slot** : Drop inactive slots:
  ```sql
  SELECT pg_drop_replication_slot('stale_slot_name');
  ```
- **Restart replication** (if broken):
  ```bash
  # On replica
  podman exec faso-postgres-replica pg_ctl -D /var/lib/postgresql/data stop
  # Re-basebackup if WAL gap too large
  pg_basebackup -h primary -D /var/lib/postgresql/data -R -Xs -P
  podman restart faso-postgres-replica
  ```

## Escalation

| Time | Action |
|------|--------|
| 0 min | Oncall acknowledges, checks replication status |
| 5 min | If lag > 5 min, stop read routing to replica immediately |
| 15 min | If not recovering, escalate to DBA oncall |
| 30 min | If replication broken, escalate to SRE lead for re-basebackup |
| 60 min | Incident commander activated if data consistency at risk |

## Prevention

- Alert on replication lag at WARNING > 30s, CRITICAL > 5 min
- Monitor WAL generation rate; alert on spikes (bulk operations should be scheduled)
- Set `hot_standby_feedback = on` to reduce query cancellation on replica
- Regular vacuum on primary to reduce WAL volume
- Capacity plan replica disk I/O to handle 2x normal WAL rate
- Schedule maintenance windows for bulk data operations

## Related Alerts

- `FasoPouletsApiDown` — read query failures if replica is behind and stale
- `FasoPostgresDiskFull` — WAL retention filling primary disk
- `FasoArmageddonLatencyHigh` — if reads routed to slow replica, gateway latency rises
