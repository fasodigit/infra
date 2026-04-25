<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# Full Stack Recovery — Runbook

**Severity** : Critical (Disaster Recovery)
**Alert** : Multiple — total outage scenario
**Oncall** : See `/observability/oncall.yml`
**SLA** : RTO 30 min (automated) / 60 min (manual)
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Total platform outage (all services down)
- Triggered by: datacenter failure, power loss, catastrophic infrastructure event
- All health endpoints returning errors or unreachable
- Status page shows all services as "Major Outage"
- No metrics flowing to Prometheus; Grafana dashboards empty

## Impact

- **User-facing** : Complete platform unavailability — no login, no orders, no notifications
- **Business** : Total revenue loss for duration of outage; regulatory notification may be required
- **SLO** : All SLOs burning at maximum rate
- **Data risk** : Depends on RPO — last backup determines potential data loss window

## Pre-Recovery Checklist

Before starting recovery, verify:

- [ ] Infrastructure is stable (power restored, network up, nodes reachable)
- [ ] SSH/kubectl access to all nodes confirmed
- [ ] Unseal keys available (`~/.faso-vault-keys.json` or key holders reachable)
- [ ] Backup locations verified (PostgreSQL WAL archive, Redpanda snapshots)
- [ ] Incident commander designated and communication channel open (`#faso-incident`)
- [ ] Status page updated: "We are aware of the issue and working on recovery"

---

## Recovery Procedure

**CRITICAL: Follow this order exactly. Each step depends on the previous ones.**

### Phase 1: Infrastructure Foundation (0-10 min)

#### Step 1: Consul cluster (3 nodes)

```bash
# Start all Consul nodes
podman-compose -f podman-compose.yml -f ../../vault/podman-compose.vault.yml up -d consul

# Wait for leader election (max 60s)
for i in $(seq 1 60); do
  LEADER=$(curl -s http://localhost:8500/v1/status/leader 2>/dev/null)
  if [ "$LEADER" != '""' ] && [ -n "$LEADER" ]; then
    echo "[OK] Consul leader elected: $LEADER"
    break
  fi
  echo "Waiting for Consul leader... ($i/60)"
  sleep 1
done

# Verify cluster health
curl -s http://localhost:8500/v1/agent/members | jq '.[].Status'
# Expected: all members Status=1 (alive)
```

**Verification** :
- [ ] Consul leader elected
- [ ] All Consul members alive
- [ ] KV store accessible: `curl -s http://localhost:8500/v1/kv/?recurse | jq length`

#### Step 2: Vault (unseal + verify KV engine)

```bash
# Start Vault
podman-compose -f podman-compose.yml -f ../../vault/podman-compose.vault.yml up -d vault

# Wait for Vault to start (10s)
sleep 10

# Unseal
export VAULT_ADDR=http://localhost:8200
KEYS_FILE=~/.faso-vault-keys.json
vault operator unseal $(jq -r '.unseal_keys_b64[0]' "$KEYS_FILE")
vault operator unseal $(jq -r '.unseal_keys_b64[1]' "$KEYS_FILE")
vault operator unseal $(jq -r '.unseal_keys_b64[2]' "$KEYS_FILE")

# Verify
vault status | grep "Sealed.*false"
export VAULT_TOKEN=$(jq -r .root_token "$KEYS_FILE")
vault kv list faso/
```

**Verification** :
- [ ] Vault unsealed (`Sealed: false`)
- [ ] KV engine accessible (`vault kv list faso/` returns entries)
- [ ] All secret paths readable: `for svc in auth-ms poulets-api notifier-ms armageddon kaya; do vault kv get faso/$svc/db 2>&1 | head -1; done`

### Phase 2: Data Layer (5-15 min)

#### Step 3: PostgreSQL primary

```bash
# Start PostgreSQL
podman-compose -f podman-compose.yml up -d postgres

# Wait for ready
for i in $(seq 1 30); do
  pg_isready -h localhost -p 5432 && echo "[OK] PostgreSQL primary ready" && break
  echo "Waiting for PostgreSQL... ($i/30)"
  sleep 2
done

# If restoring from backup (data loss scenario)
# pg_restore -h localhost -p 5432 -U postgres -d faso /backups/latest/faso.dump

# Verify WAL integrity
psql -h localhost -p 5432 -U postgres -c "SELECT pg_current_wal_lsn();"
```

**Verification** :
- [ ] PostgreSQL accepting connections
- [ ] WAL position returned (no corruption)
- [ ] All databases present: `psql -h localhost -p 5432 -U postgres -l`

#### Step 4: PostgreSQL replicas

```bash
# Start replicas (if applicable)
podman-compose -f podman-compose.yml up -d postgres-replica

# Wait for replication to sync
for i in $(seq 1 60); do
  LAG=$(psql -h localhost -p 5432 -U postgres -t -c "SELECT COALESCE(MAX(replay_lag), '0s') FROM pg_stat_replication;")
  echo "Replication lag: $LAG"
  # If lag < 1s, consider synced
  sleep 2
done
```

**Verification** :
- [ ] Replica connected to primary (`pg_stat_replication` shows streaming)
- [ ] Replication lag < 1s

#### Step 5: KAYA

```bash
# Start KAYA
podman-compose -f podman-compose.yml up -d kaya

# Wait for WAL replay
podman logs -f faso-kaya 2>&1 | grep -m1 "ready to accept connections"

# Verify
podman exec faso-kaya kaya-cli ping
podman exec faso-kaya kaya-cli INFO memory
podman exec faso-kaya kaya-cli DBSIZE
```

**Verification** :
- [ ] KAYA responds to PING
- [ ] WAL replay completed (check logs for "WAL replay finished")
- [ ] Key count reasonable (compare with pre-outage baseline)

#### Step 6: Redpanda cluster

```bash
# Start Redpanda
podman-compose -f podman-compose.yml up -d redpanda

# Wait for cluster formation
for i in $(seq 1 60); do
  HEALTH=$(rpk cluster health 2>/dev/null)
  echo "$HEALTH" | grep -q "HEALTHY" && echo "[OK] Redpanda healthy" && break
  echo "Waiting for Redpanda... ($i/60)"
  sleep 2
done

# Verify ISR (In-Sync Replicas) for critical topics
rpk topic describe poulets.orders.v1
rpk topic describe notifications.events.v1
```

**Verification** :
- [ ] All brokers alive: `rpk cluster info`
- [ ] Cluster health: `HEALTHY`
- [ ] Critical topics have full ISR

### Phase 3: Identity and Auth (10-20 min)

#### Step 7: ORY Kratos + Keto

```bash
# Start ORY services
podman-compose -f podman-compose.yml up -d kratos keto

# Wait for Kratos migration
podman logs -f faso-kratos 2>&1 | grep -m1 "migrations.*applied\|ready"

# Verify Kratos
curl -s http://localhost:4434/health/ready | jq .
curl -s http://localhost:4434/health/alive | jq .

# Verify Keto
curl -s http://localhost:4466/health/ready | jq .
```

**Verification** :
- [ ] Kratos healthy (ready + alive)
- [ ] Keto healthy (ready)
- [ ] Kratos migrations applied: `podman logs faso-kratos | grep migration`

#### Step 8: auth-ms

```bash
# Start auth-ms
podman-compose -f podman-compose.yml up -d auth-ms

# Wait for Spring Boot startup
for i in $(seq 1 60); do
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:9002/actuator/health 2>/dev/null)
  [ "$STATUS" = "200" ] && echo "[OK] auth-ms ready" && break
  echo "Waiting for auth-ms... ($i/60)"
  sleep 2
done
```

**Verification** :
- [ ] Actuator health UP: `curl -s http://localhost:9002/actuator/health | jq .status`
- [ ] DB connectivity OK: `curl -s http://localhost:9002/actuator/health/db | jq .status`
- [ ] KAYA connected: `curl -s http://localhost:9002/actuator/health/kaya | jq .status`
- [ ] HTTP port 8801 responding
- [ ] gRPC port 8702 responding

### Phase 4: Application Layer (15-25 min)

#### Step 9: poulets-api

```bash
# Start poulets-api
podman-compose -f podman-compose.yml up -d poulets-api

# Wait for readiness
for i in $(seq 1 60); do
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:9001/actuator/health 2>/dev/null)
  [ "$STATUS" = "200" ] && echo "[OK] poulets-api ready" && break
  echo "Waiting for poulets-api... ($i/60)"
  sleep 2
done
```

**Verification** :
- [ ] Actuator health UP
- [ ] GraphQL endpoint responding: `curl -s http://localhost:8901/graphql -d '{"query":"{__typename}"}' | jq .`
- [ ] DB connectivity OK
- [ ] KAYA cache connected

#### Step 10: notifier-ms

```bash
# Start notifier-ms
podman-compose -f podman-compose.yml up -d notifier-ms

# Wait for readiness
for i in $(seq 1 60); do
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:9003/actuator/health 2>/dev/null)
  [ "$STATUS" = "200" ] && echo "[OK] notifier-ms ready" && break
  echo "Waiting for notifier-ms... ($i/60)"
  sleep 2
done
```

**Verification** :
- [ ] Actuator health UP
- [ ] Redpanda consumer groups registered: `rpk group describe notifier-consumer-group`
- [ ] SMTP connectivity OK (MailHog or production)

#### Step 11: ARMAGEDDON

```bash
# Start ARMAGEDDON
podman-compose -f podman-compose.yml up -d armageddon

# Wait for readiness
for i in $(seq 1 30); do
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:9902/admin/health 2>/dev/null)
  [ "$STATUS" = "200" ] && echo "[OK] ARMAGEDDON ready" && break
  echo "Waiting for ARMAGEDDON... ($i/30)"
  sleep 2
done
```

**Verification** :
- [ ] Admin health OK on port 9902
- [ ] All upstreams healthy: `curl -s http://localhost:9903/admin/clusters | jq '.[] | {name, healthy_count}'`
- [ ] HTTP proxy on port 8080 responding
- [ ] Rate limiting active: `curl -s http://localhost:9903/admin/stats | jq .rate_limit_active`

#### Step 12: poulets-bff + frontend

```bash
# Start BFF and frontend
podman-compose -f podman-compose.yml up -d bff frontend

# Wait for readiness
for i in $(seq 1 30); do
  BFF=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:4800/health 2>/dev/null)
  FE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:4801/ 2>/dev/null)
  [ "$BFF" = "200" ] && [ "$FE" = "200" ] && echo "[OK] BFF + Frontend ready" && break
  echo "Waiting for BFF ($BFF) + Frontend ($FE)... ($i/30)"
  sleep 2
done
```

**Verification** :
- [ ] BFF health OK on port 4800
- [ ] Frontend serving HTML on port 4801
- [ ] End-to-end test: login page loads

### Phase 5: Observability (25-30 min)

#### Step 13: Observability stack

```bash
# Start observability
podman-compose -f podman-compose.yml -f ../../observability/grafana/podman-compose.observability.yml up -d prometheus loki tempo grafana otel-collector

# Wait for Prometheus
for i in $(seq 1 30); do
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:9090/-/ready 2>/dev/null)
  [ "$STATUS" = "200" ] && echo "[OK] Prometheus ready" && break
  sleep 2
done

# Wait for Grafana
for i in $(seq 1 30); do
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/api/health 2>/dev/null)
  [ "$STATUS" = "200" ] && echo "[OK] Grafana ready" && break
  sleep 2
done
```

**Verification** :
- [ ] Prometheus scraping targets: `curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets | length'`
- [ ] Loki receiving logs: `curl -s http://localhost:3101/ready`
- [ ] Tempo ready: `curl -s http://localhost:3200/ready`
- [ ] Grafana accessible: `curl -s http://localhost:3000/api/health`
- [ ] OTel collector forwarding: `curl -s http://localhost:4318/health`

---

## Post-Recovery Validation (30-45 min)

### End-to-End Smoke Tests

```bash
# 1. Health check all services
for svc in "localhost:8801" "localhost:8901" "localhost:8803" "localhost:4800" "localhost:4801" "localhost:8080"; do
  STATUS=$(curl -s -o /dev/null -w "%{http_code}" "http://$svc/health" 2>/dev/null)
  echo "$svc: $STATUS"
done

# 2. Authentication flow
curl -s http://localhost:4433/self-service/login/api | jq .id

# 3. Catalog access
curl -s http://localhost:8901/graphql -H "Content-Type: application/json" \
  -d '{"query":"{ products(first: 5) { edges { node { id name } } } }"}' | jq .

# 4. KAYA session test
podman exec faso-kaya kaya-cli SET test:recovery:$(date +%s) "ok" EX 60
podman exec faso-kaya kaya-cli GET test:recovery:*

# 5. Event pipeline test (produce + consume)
echo '{"test":"recovery","ts":"'$(date -u +%FT%TZ)'"}' | rpk topic produce test.recovery.v1
rpk topic consume test.recovery.v1 --num 1 --offset end
```

### Verify Metrics Are Flowing

```bash
# Check Prometheus has recent data (last 5 min)
curl -s "http://localhost:9090/api/v1/query?query=up" | jq '.data.result | length'
# Should return count of all scraped targets
```

### Update Status Page

```bash
# Mark recovery complete
# Update status.faso.gov.bf — all services operational
# Post incident timeline in #faso-incident channel
```

---

## Recovery Timing Summary

| Phase | Components | Estimated Time | Cumulative |
|-------|-----------|----------------|------------|
| 1 — Foundation | Consul, Vault | 5-10 min | 10 min |
| 2 — Data | PostgreSQL, KAYA, Redpanda | 5-10 min | 20 min |
| 3 — Identity | Kratos, Keto, auth-ms | 3-5 min | 25 min |
| 4 — Application | poulets-api, notifier, ARMAGEDDON, BFF, frontend | 5-10 min | 30 min |
| 5 — Observability | Prometheus, Loki, Tempo, Grafana | 3-5 min | 35 min |
| Validation | Smoke tests | 10-15 min | 45 min |

**Total RTO** : ~30 min (automated with scripts) / ~60 min (manual step-by-step)

## Escalation

| Time | Action |
|------|--------|
| 0 min | Incident commander designated, all-hands page sent |
| 5 min | Recovery started, status page updated |
| 30 min | If Phase 2 not complete, escalate to DBA + platform lead |
| 45 min | If Phase 4 not complete, all engineering managers paged |
| 60 min | CTO notified; public communication if external SLA impacted |
| 120 min | Regulatory notification if data loss confirmed |

## Post-Incident

- [ ] Full postmortem within 48h (see `postmortem-workflow.md`)
- [ ] Status page incident resolved with full timeline
- [ ] All action items tracked in GitHub Issues
- [ ] Recovery runbook updated with any new findings
- [ ] Schedule DR drill for next quarter

## Related Runbooks

- `vault-sealed.md` — Step 2 details
- `postgres-replication-lag.md` — Step 4 details
- `kaya-down.md` — Step 5 details
- `redpanda-partition-offline.md` — Step 6 details
- `certificate-rotation-failed.md` — mTLS recovery if SPIRE involved
