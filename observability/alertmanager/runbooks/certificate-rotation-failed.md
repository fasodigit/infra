<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# FasoSvidExpiryCritical — Runbook

**Severity** : Critical
**Alert** : `FasoSvidExpiryCritical`
**Oncall** : See `/observability/oncall.yml`
**SLA** : diagnostic + mitigation < 10 min
**Derniere mise a jour** : 2026-04-24

---

## Symptoms

- Alert `FasoSvidExpiryCritical` firing (SVID expiry < 24h or already expired)
- mTLS handshakes failing between services (connection reset, TLS alert)
- Inter-service HTTP calls returning 503 (upstream connection failure)
- ARMAGEDDON logs showing `tls handshake error` for upstream connections
- SPIRE agent logs showing `failed to renew SVID` or `failed to attest`
- New pod registrations failing (cannot obtain identity)

## Impact

- **User-facing** : Service-to-service calls failing — appears as 503 errors, timeouts, or partial responses
- **Business** : All internal mesh communication at risk; cascading failures across the platform
- **SLO** : All service SLOs impacted via availability and error rate burn
- **Security** : Expired certificates mean no mTLS verification — services cannot prove identity

## Diagnosis

### Step 1: Check SVID expiry time

```bash
# From inside a service pod
kubectl exec -n faso deploy/auth-ms -- \
  spire-agent api fetch x509 -socketPath /run/spire/sockets/agent.sock 2>&1 | grep -i "expiry\|not after"

# Check TTL directly
kubectl exec -n faso deploy/auth-ms -- \
  openssl x509 -in /tmp/spire/svid.pem -noout -enddate 2>/dev/null
```

Expected: `notAfter` should be at least 24h in the future. SVID default TTL is 24h, rotation at 50% lifetime (12h).

### Step 2: Check SPIRE agent health

```bash
kubectl get pods -l app=spire-agent -n spire
kubectl logs -l app=spire-agent -n spire --tail=100 | grep -iE "error|renew|attest|expired"

# Agent health check
kubectl exec -n spire ds/spire-agent -- \
  /opt/spire/bin/spire-agent healthcheck -socketPath /run/spire/sockets/agent.sock
```

### Step 3: Check SPIRE server health

```bash
kubectl get pods -l app=spire-server -n spire
kubectl logs -l app=spire-server -n spire --tail=100 | grep -iE "error|ca|signing|expire"

# Server health check
kubectl exec -n spire deploy/spire-server -- \
  /opt/spire/bin/spire-server healthcheck
```

### Step 4: Check trust bundle

```bash
# Fetch trust bundle
kubectl exec -n spire deploy/spire-server -- \
  /opt/spire/bin/spire-server bundle show -format pem | openssl x509 -noout -enddate
```

If root CA is expired, ALL SVIDs are invalid. This is a critical event.

### Step 5: Check clock skew

```bash
# Compare time across nodes
kubectl get nodes -o wide
for node in $(kubectl get nodes -o name); do
  echo "$node: $(kubectl debug $node --image=busybox -- date -u 2>/dev/null)"
done

# Local dev
date -u
podman exec faso-auth-ms date -u
```

Clock skew > 30s can cause certificate validation failures.

### Step 6: List SPIRE registrations

```bash
kubectl exec -n spire deploy/spire-server -- \
  /opt/spire/bin/spire-server entry show
```

Verify all services have entries. Missing entries prevent SVID issuance.

## Remediation

### Quick Fix (< 5 min)

1. **Restart SPIRE agent** (triggers immediate SVID renewal):
   ```bash
   kubectl -n spire rollout restart ds/spire-agent
   # Wait for agents to be ready
   kubectl -n spire rollout status ds/spire-agent --timeout=120s
   ```

2. **Force SVID rotation** (if agent restart is insufficient):
   ```bash
   # Delete cached SVIDs to force re-issuance
   kubectl exec -n faso deploy/auth-ms -- rm -f /tmp/spire/svid.pem /tmp/spire/key.pem
   # Restart the affected service (it will request new SVID on startup)
   kubectl rollout restart deploy/auth-ms -n faso
   ```

3. **Manual certificate issue** (emergency fallback if SPIRE is fully down):
   ```bash
   # Issue cert from Vault PKI (emergency only)
   vault write faso-pki/issue/mesh-emergency \
     common_name="auth-ms.faso.internal" \
     ttl="24h"
   ```
   **Warning**: Manual certs bypass SPIRE identity verification. Use only as last resort.

### Root Cause Fix

- **SPIRE server CA expired** :
  ```bash
  # Rotate upstream CA
  kubectl exec -n spire deploy/spire-server -- \
    /opt/spire/bin/spire-server bundle set -format pem < new-ca.pem
  # Restart all agents to pick up new trust bundle
  kubectl -n spire rollout restart ds/spire-agent
  ```

- **Clock skew** : Fix NTP on affected nodes:
  ```bash
  # Check NTP status
  timedatectl status
  # Force NTP sync
  sudo systemctl restart chronyd
  ```

- **SPIRE agent attestation failure** : Re-register the node:
  ```bash
  kubectl exec -n spire deploy/spire-server -- \
    /opt/spire/bin/spire-server entry create \
    -spiffeID spiffe://faso.internal/ns/faso/sa/auth-ms \
    -parentID spiffe://faso.internal/k8s-node \
    -selector k8s:sa:auth-ms \
    -selector k8s:ns:faso
  ```

### Check trust domain configuration

```bash
kubectl exec -n spire deploy/spire-server -- \
  /opt/spire/bin/spire-server bundle show | head -5
```

Trust domain must be `faso.internal`. Mismatch causes all validation to fail.

## Escalation

| Time | Action |
|------|--------|
| 0 min | Oncall acknowledges, restarts SPIRE agent |
| 5 min | If SVIDs not renewing, check SPIRE server |
| 10 min | If root CA issue, escalate to security team immediately |
| 15 min | If not resolved, page SRE lead + platform team |
| 30 min | Incident commander activated — mesh communication at risk |

## Prevention

- Alert on SVID expiry at WARNING < 72h, CRITICAL < 24h (not just at 24h)
- Monitor SPIRE agent health with synthetic attestation check every 5 min
- Automated root CA rotation (Vault PKI integrated with SPIRE)
- NTP monitoring: alert on clock skew > 10s across all nodes
- SPIRE server HA: minimum 3 replicas with leader election
- Quarterly disaster recovery drill: simulate CA expiry and practice rotation

## Related Alerts

- `FasoArmageddonLatencyHigh` — mTLS failure looks like upstream failure
- `FasoAuthMsDown` — service cannot communicate over mesh
- `FasoPouletsApiDown` — service cannot communicate over mesh
- `FasoVaultSealed` — Vault PKI engine may be the upstream CA source
