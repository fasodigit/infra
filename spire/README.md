<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# FASO DIGITALISATION — SPIRE / SPIFFE

Trust domain : **`faso.gov.bf`**  
SVID TTL     : 24 h (workloads) — rotated automatically 12 h before expiry  
Node TTL     : 1 h  
Toolchain    : SPIRE 1.9.6 — podman-compose (rootless, no daemon)

---

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                     faso.gov.bf                          │
│                                                          │
│  ┌─────────────────┐       ┌──────────────────────────┐  │
│  │  SPIRE Server   │       │      SPIRE Agent         │  │
│  │  :8081 (API)    │◄─────►│  /run/spire/sockets/     │  │
│  │  :9988 (prom)   │       │  agent.sock (Workload API)│  │
│  │  Postgres store │       │  :9989 (prom)            │  │
│  └─────────────────┘       └────────────┬─────────────┘  │
│                                         │ X.509 SVID     │
│                    ┌────────────────────┼──────────────┐ │
│                    │  Workloads         │              │ │
│                    │  ARMAGEDDON ───────┘              │ │
│                    │  KAYA             (via            │ │
│                    │  auth-ms           armageddon-    │ │
│                    │  poulets-api       mesh crate)    │ │
│                    │  notifier-ms                      │ │
│                    │  etat-civil-ms                    │ │
│                    │  sogesy-ms                        │ │
│                    │  frontend-bff                     │ │
│                    └───────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

**Attestation (dev)**: join_token (server) + unix:uid (workload)  
**Attestation (prod)**: k8s_psat (node) + k8s service account (workload)

SPIFFE IDs follow the scheme `spiffe://faso.gov.bf/ns/<namespace>/sa/<name>`.

---

## Registered workloads

| SPIFFE ID | Service | Port |
|-----------|---------|------|
| `spiffe://faso.gov.bf/ns/default/sa/armageddon` | ARMAGEDDON gateway (Rust) | 8443 |
| `spiffe://faso.gov.bf/ns/default/sa/kaya` | KAYA database (Rust) | 6380 |
| `spiffe://faso.gov.bf/ns/default/sa/auth-ms` | auth microservice (Java 21) | 8080 |
| `spiffe://faso.gov.bf/ns/default/sa/poulets-api` | poulets platform API (Java 21) | 8081 |
| `spiffe://faso.gov.bf/ns/default/sa/notifier-ms` | notification microservice (Java 21) | 8082 |
| `spiffe://faso.gov.bf/ns/default/sa/etat-civil-ms` | état civil microservice (Java 21) | 8083 |
| `spiffe://faso.gov.bf/ns/default/sa/sogesy-ms` | SOGESY microservice (Java 21) | 8084 |
| `spiffe://faso.gov.bf/ns/default/sa/frontend-bff` | Angular BFF (Node/Rust) | 4200 |

---

## Dev workflow — bootstrap local

```bash
cd INFRA/spire

# Full bootstrap (idempotent — safe to re-run):
bash scripts/bootstrap.sh
```

This script:
1. Generates a self-signed CA under `certs/` (P-256, 90-day validity).
2. Creates the `faso-net` podman network if absent.
3. Starts `faso-spire-server` (waits for healthy).
4. Generates a join token (TTL 3600 s) and starts `faso-spire-agent`.
5. Registers the 8 workload entries with `TTL=86400s` and `unix:uid:<current_uid>` selector.

After bootstrap:
```bash
# Verify entries
podman exec faso-spire-server /opt/spire/bin/spire-server entry show

# Fetch SVID for armageddon (socket exposed via spire-sockets volume)
podman exec faso-spire-agent \
  /opt/spire/bin/spire-agent api fetch x509 \
  -socketPath /run/spire/sockets/agent.sock

# Inspect expiry
bash scripts/check-expiration.sh
```

---

## Prod workflow — automatic rotation (no manual action needed)

SPIRE rotates SVIDs automatically 12 h before expiry:
- Agent polls the server at the `trust_bundle_refresh_hint` interval (1 h).
- Workloads receive the new SVID via the Workload API stream (push).
- Rust workloads (`armageddon-mesh` crate) reload `rustls::ServerConfig` via `ArcSwap` — **no restart**.
- Java workloads use `spiffe-java-sdk` `X509SourceBean` `onUpdate` callback — no restart.

Manual force-rotation (if needed):
```bash
# On the server: delete + recreate the entry — agent will push new SVID immediately
podman exec faso-spire-server \
  /opt/spire/bin/spire-server entry delete -id <entry-id>
# Then re-register via bootstrap.sh (idempotent)
bash scripts/bootstrap.sh
```

---

## Expiration monitoring

`scripts/check-expiration.sh` is called:
- **Every hour** by `.github/workflows/spire-expiration-check.yml` (cron `17 * * * *`).
- Pushes `spire_svid_expires_in_hours{spiffe_id="..."}` to Prometheus Pushgateway.
- Exit code 1 → CI fails → GitHub issue P1 is created automatically.

Prometheus alerts (`observability/alertmanager/rules/spire.yml`):
| Alert | Condition | Duration | Route |
|-------|-----------|----------|-------|
| `SpireSvidExpiringCritical` | `< 24 h` | 10 min | PagerDuty (P1) |
| `SpireSvidExpiringWarn` | `< 72 h` | 1 h | Slack #faso-ops-alerts |
| `SpireAgentDown` | `up{job="spire-agent"} == 0` | 2 min | PagerDuty (P1) |
| `SpireServerDown` | `up{job="spire-server"} == 0` | 2 min | PagerDuty (P1) |

---

## Rust integration (ARMAGEDDON / KAYA)

Both services consume SVIDs via the `armageddon-mesh` crate (already delivered).
No modification is needed — the crate reads from `/run/spire/sockets/agent.sock`:

```toml
# Cargo.toml — already present in armageddon-mesh
spiffe = "0.4"
arc-swap = "1"
```

The workload container must mount the `spire-sockets` podman volume:
```yaml
volumes:
  - spire-sockets:/run/spire/sockets:ro,Z
```

---

## Java integration (auth-ms, poulets-api, notifier-ms, etat-civil-ms, sogesy-ms)

```xml
<dependency>
  <groupId>io.spiffe</groupId>
  <artifactId>spiffe-java-sdk</artifactId>
  <version>0.8.3</version>
</dependency>
```

```java
X509Source source = X509Source.newSource(
    X509SourceOptions.builder()
        .workloadApiAddress("unix:/run/spire/sockets/agent.sock")
        .build()
);
// Reload SslContext on every onUpdate callback (zero-restart rotation)
```

---

## Runbooks

### runbook-svid-expire

**Symptom**: `SpireSvidExpiringCritical` fires or CI check exits 1.

**Root causes and resolution**:

1. **SPIRE agent disconnected from server** (most common):
   ```bash
   podman logs faso-spire-agent | tail -50
   # Look for "failed to establish connection" or "attestation failed"
   podman restart faso-spire-agent
   ```

2. **SPIRE server Postgres unreachable**:
   ```bash
   podman exec faso-spire-server \
     /opt/spire/bin/spire-server healthcheck
   # If unhealthy, check faso-postgres connectivity
   podman logs faso-spire-server | grep -i "sql\|postgres\|error"
   ```

3. **Corrupted trust bundle / CA rotation**:
   ```bash
   # Re-bootstrap CA and force re-attestation
   rm -f INFRA/spire/certs/ca.*
   bash INFRA/spire/scripts/bootstrap.sh
   ```

4. **Entry deleted or TTL misconfigured**:
   ```bash
   podman exec faso-spire-server \
     /opt/spire/bin/spire-server entry show
   # Re-run bootstrap.sh to recreate missing entries
   bash INFRA/spire/scripts/bootstrap.sh
   ```

5. **Clock skew between server and agent host** (> 1 h):
   ```bash
   timedatectl status
   chronyc tracking
   # Fix NTP, then restart both containers
   ```

---

### runbook-agent-down

**Symptom**: `SpireAgentDown` fires — `up{job="spire-agent"} == 0`.

1. Check podman container state:
   ```bash
   podman ps -a | grep spire-agent
   podman logs faso-spire-agent --tail 100
   ```
2. If container exited: `podman start faso-spire-agent`.
3. If container is running but metrics unreachable: check port 9989 binding.
4. If join token expired (dev only): re-run `bash scripts/bootstrap.sh`.

---

### runbook-server-down

**Symptom**: `SpireServerDown` fires.

1. `podman ps -a | grep spire-server` — restart if exited.
2. Check Postgres connectivity: `${SPIRE_POSTGRES_URL}` env var must be set.
3. Check CA cert/key files in `certs/` are present and readable.
4. Review logs: `podman logs faso-spire-server --tail 100`.

---

## File reference

| File | Purpose |
|------|---------|
| `server/server.conf` | SPIRE server HCL config (TTL 86400s, Postgres, disk KeyManager) |
| `agent/agent.conf` | SPIRE agent HCL config (unix+k8s+docker WorkloadAttestors) |
| `podman-compose.spire.yml` | podman-compose stack (server + agent, faso-net, healthchecks) |
| `scripts/bootstrap.sh` | Dev bootstrap: start stack + generate join token + register 8 entries |
| `scripts/check-expiration.sh` | Hourly expiry check + Pushgateway push, exit 1 if < 24 h |
| `certs/` | Self-signed CA (dev, gitignored) |
| `../../observability/alertmanager/rules/spire.yml` | Prometheus alerting rules |
| `../../.github/workflows/spire-expiration-check.yml` | Hourly CI cron, creates P1 issue on failure |
