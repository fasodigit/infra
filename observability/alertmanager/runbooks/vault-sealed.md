<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# FasoVaultSealed — Runbook

**Severity** : Critical
**Alert** : `FasoVaultSealed`
**Oncall** : See `/observability/oncall.yml`
**SLA** : diagnostic + mitigation < 5 min (URGENT — blocks all deployments)
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Alert `FasoVaultSealed` firing in Alertmanager
- `vault status` returns `Sealed: true`
- Services failing to start with "secret not found" or "vault connection refused"
- New pod deployments stuck in `Init` container phase (waiting for Vault Agent sidecar)
- Vault UI shows sealed status (port 8200)
- No secret rotation occurring (dynamic DB creds not being refreshed)

## Impact

- **User-facing** : No immediate impact if running services have cached secrets; NEW deployments and restarts will fail
- **Business** : Platform cannot deploy new code; secret rotation stops; compliance risk if creds expire
- **SLO** : No direct SLO impact initially, but cascading failures within secret TTL window (default 768h for static, 1h for dynamic DB creds)
- **Blast radius** : ALL services affected — auth-ms, poulets-api, notifier-ms, ARMAGEDDON, KAYA

## Diagnosis

### Step 1: Check Vault status

```bash
export VAULT_ADDR=http://localhost:8200

vault status
```

Expected output when sealed:
```
Sealed          true
Total Shares    5
Threshold       3
Unseal Progress 0/3
```

### Step 2: Check Vault container health

```bash
podman ps --filter name=faso-vault
podman logs --tail 100 faso-vault | grep -iE "seal|error|panic|consul"

# Kubernetes
kubectl get pods -l app=vault -n faso
kubectl describe pod -l app=vault -n faso | tail -30
```

### Step 3: Check Consul storage backend

```bash
curl -s http://localhost:8500/v1/status/leader
# Expected: IP:port of Consul leader

curl -s http://localhost:8500/v1/health/service/vault | jq '.[].Status'
# Expected: "passing"

podman ps --filter name=faso-consul
podman logs --tail 50 faso-consul | grep -iE "error|leader|election"
```

If Consul leader election is failing, Vault cannot access its storage.

### Step 4: Check auto-unseal mechanism

```bash
podman logs --tail 200 faso-vault | grep -i "auto.unseal\|kms\|transit\|seal.*error"
```

Auto-unseal failure reasons:
- Transit Vault unreachable
- KMS key deleted or access revoked
- Network partition to auto-unseal provider

### Step 5: Check audit log for seal events

```bash
# Vault audit log (if file audit enabled)
podman exec faso-vault cat /vault/logs/audit.log | tail -50 | jq 'select(.type == "system") | {time: .time, operation: .request.operation, error: .error}'
```

## Remediation

### Quick Fix: Manual unseal (< 2 min)

```bash
export VAULT_ADDR=http://localhost:8200

# Retrieve unseal keys (NEVER stored in repo — on operator machine only)
# Keys file created by INFRA/vault/scripts/init.sh
KEYS_FILE=~/.faso-vault-keys.json

# Unseal (need threshold number of keys, default 3 of 5)
vault operator unseal $(jq -r '.unseal_keys_b64[0]' "$KEYS_FILE")
vault operator unseal $(jq -r '.unseal_keys_b64[1]' "$KEYS_FILE")
vault operator unseal $(jq -r '.unseal_keys_b64[2]' "$KEYS_FILE")

# Verify
vault status | grep "Sealed"
# Expected: Sealed false
```

### Fix auto-unseal (if auto-unseal was configured)

1. **Transit auto-unseal** (another Vault instance):
   ```bash
   # Check transit Vault health
   curl -s http://transit-vault:8200/v1/sys/health | jq .
   # If transit Vault is sealed, unseal it first
   ```

2. **Restart Vault** (may trigger auto-unseal on startup):
   ```bash
   podman restart faso-vault
   # Wait 10s then check
   vault status
   ```

### Fix Consul storage

If Consul is the root cause:

```bash
# Restart Consul
podman restart faso-consul

# Wait for leader election (30s timeout)
for i in $(seq 1 30); do
  LEADER=$(curl -s http://localhost:8500/v1/status/leader)
  if [ "$LEADER" != '""' ] && [ -n "$LEADER" ]; then
    echo "Consul leader elected: $LEADER"
    break
  fi
  sleep 1
done

# Then restart Vault (it should auto-unseal or manual unseal)
podman restart faso-vault
```

### Post-unseal verification

```bash
# 1. Vault health
vault status
curl -s http://localhost:8200/v1/sys/health | jq .

# 2. Verify KV engine accessible
export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
vault kv list faso/

# 3. Check services can fetch secrets
vault kv get faso/auth-ms/db
vault kv get faso/poulets-api/db

# 4. Restart any pods stuck in Init
kubectl delete pod -l vault-agent-injector=true --field-selector=status.phase=Pending -n faso
```

## Escalation

| Time | Action |
|------|--------|
| 0 min | Oncall acknowledges, attempts manual unseal immediately |
| 5 min | If manual unseal fails (key holders unavailable), escalate to SRE lead |
| 10 min | If Consul storage issue, page platform team |
| 15 min | If not resolved, page engineering manager |
| 30 min | Incident commander activated — no new deployments possible |

## Prevention

- Configure auto-unseal (Transit or KMS) to avoid manual intervention
- Monitor `vault_core_unsealed` metric with alert on value `0`
- Ensure at least 3 key holders are reachable 24/7 (if Shamir unseal)
- Test unseal procedure quarterly (disaster recovery drill)
- Consul storage: minimum 3-node cluster with anti-affinity for quorum safety
- Document key holder contact list in secure, offline medium

## Security Reminders

- NEVER store unseal keys in the git repository
- NEVER store unseal keys in the same location as Vault data
- Key holder rotation: when a team member leaves, re-key Vault (`vault operator rekey`)
- `~/.faso-vault-keys.json` must be `chmod 600` and on encrypted disk

## Related Alerts

- `FasoAuthMsDown` — auth-ms fails if Vault secrets expire and cannot be refreshed
- `FasoPouletsApiDown` — poulets-api fails if DB dynamic creds expire
- `FasoSvidExpiryCritical` — Vault PKI engine issues certs; sealed Vault stops rotation
- `FasoConsulLeaderLost` — Consul storage backend issue triggers Vault seal
