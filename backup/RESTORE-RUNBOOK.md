# SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
# SPDX-License-Identifier: AGPL-3.0-or-later

# FASO DIGITALISATION -- Disaster Recovery Restore Runbook

## SLA Targets

| Metric | Target | Notes |
|--------|--------|-------|
| **RPO** (Recovery Point Objective) | **5 minutes** (PostgreSQL), **1 hour** (KAYA), **24 hours** (Vault/Consul) | PostgreSQL WAL archiving every 5 min; KAYA hourly snapshots; Vault/Consul daily |
| **RTO** (Recovery Time Objective) | **< 2 hours** | Full stack from cold start, assuming S3 is accessible |

## Recovery Order

Restore services in this exact order. Each step depends on the previous one.

```
1. Consul     (Vault storage backend)
2. Vault      (secrets for all services)
3. PostgreSQL (primary data store)
4. KAYA       (in-memory cache/state)
5. Services   (Kratos, Keto, auth-ms, poulets-api, notifier-ms, BFF, frontend)
```

---

## Prerequisites

- Access to S3-compatible storage (OVH Object Storage) with backup bucket
- `age` private key file (stored offline, NOT in Vault -- the decryption key
  for backup encryption is kept in a secure physical location)
- `s3cmd` configured with access credentials
- `podman` (local) or `kubectl` (Kubernetes) access
- Vault unseal keys (from `~/.faso-vault-keys.json` or physical key ceremony)

Environment setup:

```bash
export S3_BUCKET="faso-backups"
export S3_ENDPOINT="s3.gra.io.cloud.ovh.net"
export BACKUP_DECRYPTION_KEY_FILE="/secure/faso-backup.age.key"
```

---

## Step 1: Restore Consul

### 1.1 Download latest Consul snapshot

```bash
# Find latest snapshot
s3cmd ls "s3://${S3_BUCKET}/faso/consul/daily/" \
  --host="${S3_ENDPOINT}" --recursive | sort -k1,2 | tail -5

# Download
s3cmd get "s3://${S3_BUCKET}/faso/consul/daily/<DATE>/consul-snapshot-<TS>.snap.age" \
  /tmp/consul-restore.snap.age \
  --host="${S3_ENDPOINT}"
```

### 1.2 Decrypt

```bash
age --decrypt \
  --identity "$BACKUP_DECRYPTION_KEY_FILE" \
  -o /tmp/consul-restore.snap \
  /tmp/consul-restore.snap.age
```

### 1.3 Verify snapshot integrity

```bash
consul snapshot inspect /tmp/consul-restore.snap
```

### 1.4 Restore

```bash
# Local (podman)
podman exec -i faso-consul consul snapshot restore /tmp/consul-restore.snap

# Kubernetes
kubectl -n faso-vault cp /tmp/consul-restore.snap consul-0:/tmp/consul-restore.snap
kubectl -n faso-vault exec consul-0 -- consul snapshot restore /tmp/consul-restore.snap
```

### 1.5 Verify

```bash
consul members
consul kv get -recurse faso/
```

---

## Step 2: Restore Vault

### 2.1 Download latest Vault snapshot

```bash
s3cmd ls "s3://${S3_BUCKET}/faso/vault/daily/" \
  --host="${S3_ENDPOINT}" --recursive | sort -k1,2 | tail -5

s3cmd get "s3://${S3_BUCKET}/faso/vault/daily/<DATE>/vault-snapshot-<TS>.snap.age" \
  /tmp/vault-restore.snap.age \
  --host="${S3_ENDPOINT}"
```

### 2.2 Decrypt

```bash
age --decrypt \
  --identity "$BACKUP_DECRYPTION_KEY_FILE" \
  -o /tmp/vault-restore.snap \
  /tmp/vault-restore.snap.age
```

### 2.3 Unseal Vault (if sealed)

```bash
# Using saved keys
KEYS_FILE="$HOME/.faso-vault-keys.json"
for i in 0 1 2; do
  vault operator unseal "$(jq -r ".keys[$i]" "$KEYS_FILE")"
done
```

### 2.4 Restore snapshot

```bash
export VAULT_ADDR="http://127.0.0.1:8200"
export VAULT_TOKEN="$(jq -r .root_token $KEYS_FILE)"

vault operator raft snapshot restore -force /tmp/vault-restore.snap
```

### 2.5 Verify

```bash
vault status
vault kv list faso/
vault kv get faso/postgres
```

---

## Step 3: Restore PostgreSQL

### 3.1 Option A: Full restore (latest weekly backup)

```bash
export PGHOST=localhost
export PGPORT=5432
export PGUSER=faso
export PGPASSWORD_FILE=/run/secrets/postgres_password

./backup/scripts/pg-restore.sh --confirm
```

### 3.2 Option B: Point-In-Time Recovery (PITR)

Restore to a specific timestamp (e.g., just before a data corruption event):

```bash
./backup/scripts/pg-restore.sh \
  --pitr "2026-04-20 14:30:00 UTC" \
  --confirm
```

### 3.3 Option C: Specific backup

```bash
./backup/scripts/pg-restore.sh \
  --s3-key "faso/postgres/weekly/2026w16/pg-full-20260420T030000Z.tar.zst.age" \
  --confirm
```

### 3.4 Verify

```bash
# Check PostgreSQL is ready
pg_isready -h localhost -p 5432 -U faso

# Check all databases exist
psql -h localhost -U faso -d faso_main -c "\l"

# Spot-check data
psql -h localhost -U faso -d faso_main -c "SELECT count(*) FROM information_schema.tables;"
psql -h localhost -U faso -d auth -c "SELECT count(*) FROM information_schema.tables;"
psql -h localhost -U faso -d poulets -c "SELECT count(*) FROM information_schema.tables;"
```

---

## Step 4: Restore KAYA

KAYA is an in-memory cache; data loss is tolerable as services will re-populate.
However, restoring accelerates recovery.

### 4.1 Download latest snapshot

```bash
s3cmd ls "s3://${S3_BUCKET}/faso/kaya/hourly/" \
  --host="${S3_ENDPOINT}" --recursive | sort -k1,2 | tail -5

s3cmd get "s3://${S3_BUCKET}/faso/kaya/hourly/<HOUR>/kaya-<TS>.rdb.zst.age" \
  /tmp/kaya-restore.rdb.zst.age \
  --host="${S3_ENDPOINT}"
```

### 4.2 Decrypt + decompress

```bash
age --decrypt \
  --identity "$BACKUP_DECRYPTION_KEY_FILE" \
  /tmp/kaya-restore.rdb.zst.age \
| zstd -d -o /tmp/kaya-restore.rdb
```

### 4.3 Stop KAYA, replace snapshot, restart

```bash
# Local (podman)
podman stop faso-kaya
cp /tmp/kaya-restore.rdb /var/lib/kaya/dump.rdb
podman start faso-kaya

# Kubernetes
kubectl -n faso-infra scale deployment faso-kaya --replicas=0
kubectl -n faso-infra cp /tmp/kaya-restore.rdb faso-kaya-0:/var/lib/kaya/dump.rdb
kubectl -n faso-infra scale deployment faso-kaya --replicas=1
```

### 4.4 Verify

```bash
kaya-cli -h localhost -p 6380 PING
kaya-cli -h localhost -p 6380 INFO keyspace
```

---

## Step 5: Restart Services

After all data stores are restored, restart application services in order:

```bash
# Local (podman-compose)
cd INFRA/docker/compose

podman-compose -f podman-compose.yml restart kratos
podman-compose -f podman-compose.yml restart keto
podman-compose -f podman-compose.yml restart auth-ms
podman-compose -f podman-compose.yml restart poulets-api
podman-compose -f podman-compose.yml restart notifier-ms
podman-compose -f podman-compose.yml restart poulets-bff
podman-compose -f podman-compose.yml restart poulets-frontend

# Kubernetes
kubectl -n faso rollout restart deployment/kratos
kubectl -n faso rollout restart deployment/keto
kubectl -n faso rollout restart deployment/auth-ms
kubectl -n faso rollout restart deployment/poulets-api
kubectl -n faso rollout restart deployment/notifier-ms
kubectl -n faso rollout restart deployment/poulets-bff
kubectl -n faso rollout restart deployment/poulets-frontend
```

---

## Verification Checklist

After full restore, verify each service:

- [ ] **Consul**: `consul members` shows all nodes
- [ ] **Vault**: `vault status` shows initialized + unsealed
- [ ] **PostgreSQL**: `pg_isready` passes, all 5 databases exist (faso_main, kratos, keto, auth, poulets)
- [ ] **KAYA**: `PING` returns `PONG`, keyspace shows expected keys
- [ ] **Kratos**: `curl http://localhost:4433/health/alive` returns 200
- [ ] **Keto**: `curl http://localhost:4466/health/alive` returns 200
- [ ] **auth-ms**: `curl http://localhost:8801/actuator/health` returns UP
- [ ] **poulets-api**: `curl http://localhost:8901/actuator/health` returns UP
- [ ] **notifier-ms**: `curl http://localhost:8803/actuator/health` returns UP
- [ ] **ARMAGEDDON**: `curl http://localhost:9090/health` returns 200
- [ ] **BFF**: `curl http://localhost:4800/api/health` returns 200
- [ ] **Frontend**: `curl http://localhost:4801/` returns 200
- [ ] **End-to-end**: login flow completes successfully

---

## Troubleshooting

### Backup stale alert

If `FasoBackupPostgresStale` or `FasoBackupVaultStale` fires:

1. Check CronJob status: `kubectl -n faso-backup get cronjobs`
2. Check recent job history: `kubectl -n faso-backup get jobs --sort-by=.status.startTime`
3. Check pod logs: `kubectl -n faso-backup logs job/faso-pg-backup-<id>`
4. Verify S3 connectivity from within the cluster
5. Verify Vault agent sidecar is injecting secrets

### Backup failure

If `FasoBackupFailed` fires:

1. Identify which backup failed (check `faso_*_backup_last_status` metrics)
2. Check pod logs for the specific CronJob
3. Common failures:
   - S3 connectivity (network policy, credentials expired)
   - Vault sealed (cannot inject secrets)
   - PostgreSQL connection refused (DB overloaded or down)
   - KAYA BGSAVE timeout (memory pressure)
   - Disk full in emptyDir (increase sizeLimit)

### PITR fails to reach target

If PostgreSQL cannot reach the PITR target timestamp:

1. Check if WAL segments are available in S3 for the target time range
2. Verify WAL archiving was continuous (no gaps in `pg-wal-archive.sh` logs)
3. The target timestamp must be between the backup creation time and the
   last archived WAL segment

---

## Escalation Contacts

| Role | Contact | When |
|------|---------|------|
| **On-call SRE** | PagerDuty rotation | First responder for all alerts |
| **Database Admin** | `fasodigitalisation@gmail.com` | PostgreSQL PITR issues, data corruption |
| **Security Lead** | `fasodigitalisation@gmail.com` | Vault restore, key ceremony |
| **Platform Lead** | `fasodigitalisation@gmail.com` | Full stack DR coordination |

---

## Backup Schedule Summary

| Service | Type | Schedule | Retention | Encryption |
|---------|------|----------|-----------|------------|
| PostgreSQL | Full (pg_basebackup) | Weekly Sun 03:00 UTC | 4 weekly + 12 monthly + 1 yearly | age |
| PostgreSQL | WAL archive | Continuous (every 5 min max) | Tied to full backup retention | age |
| KAYA | Snapshot (BGSAVE) | Hourly (:15) | 24 hourly + 7 daily | age |
| Vault | Raft snapshot | Daily 02:00 UTC | 7 daily + 4 weekly | age |
| Consul | Snapshot | Daily 02:30 UTC | 7 daily + 4 weekly | age |

All backups are stored on OVH Object Storage (S3-compatible), region GRA (Gravelines, France).
