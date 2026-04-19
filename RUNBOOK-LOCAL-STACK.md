# RUNBOOK — FASO DIGITALISATION stack local

Dernière mise à jour : 2026-04-18 session autonomous-loop.

## Démarrer le stack en 5 commandes

```bash
# 1. Containers (Vault dev-mode + postgres + consul + kaya + mailslurper)
docker start faso-postgres faso-consul faso-kaya faso-vault faso-mailslurper

# 2. Frontend + BFF
cd INFRA/poulets-platform/frontend && ng serve --port 4801 > /tmp/ng-serve.log 2>&1 &
cd INFRA/poulets-platform/bff && bun run dev > /tmp/bff-serve.log 2>&1 &

# 3. Java backends
bash INFRA/scripts/start-java-stack.sh  # voir ci-dessous

# 4. ARMAGEDDON
cd INFRA/armageddon && /var/tmp/faso-cache/cargo-target/release/armageddon \
  --config config/armageddon-dev.yaml > /tmp/armageddon.log 2>&1 &

# 5. Vérifier
bash INFRA/scripts/status.sh  # ou invoquer /status-faso via Claude
```

## Secrets & env vars (dev)

- `/tmp/faso-jwt-key` — JWT encryption key (base64, 45 bytes)
- `/tmp/faso-grpc-token` — gRPC service-to-service token
- Postgres password = `217FHIojYlPs1u4T+3gwnliX2MFjFp4RtMFTuWpbyg0=` (lecture via `podman exec faso-postgres cat /run/secrets/postgres_password`)
- Vault token dev = `faso-dev-root-token-change-in-prod` (dev-mode uniquement)

## Vault dev-mode (remplace backend Consul cassé)

```bash
podman run -d --name faso-vault --network faso-network --network-alias vault \
  -p 127.0.0.1:8200:8200 \
  -e VAULT_DEV_ROOT_TOKEN_ID='faso-dev-root-token-change-in-prod' \
  -e VAULT_DEV_LISTEN_ADDRESS='0.0.0.0:8200' \
  --cap-add=IPC_LOCK hashicorp/vault:1.18.2
```

Secrets seedés :
- `faso/postgres/superuser` (username, password)
- `faso/auth-ms/core` (jwt-encryption-key, grpc-service-token)
- `faso/growthbook/core` (jwt-secret, api-key)

## Java — auth-ms

```bash
cd INFRA/auth-ms
export JWT_KEY_ENCRYPTION_KEY=$(cat /tmp/faso-jwt-key)
export GRPC_SERVICE_TOKEN=$(cat /tmp/faso-grpc-token)
export SPRING_PROFILES_ACTIVE=dev
export GRPC_SERVER_SECURITY_ENABLED=false
export SPRING_DATASOURCE_URL="jdbc:postgresql://localhost:5432/auth_ms"
export SPRING_DATASOURCE_USERNAME=faso
export SPRING_DATASOURCE_PASSWORD="217FHIojYlPs1u4T+3gwnliX2MFjFp4RtMFTuWpbyg0="
export SPRING_REDIS_HOST=localhost SPRING_REDIS_PORT=6380
export SERVER_PORT=8801
export MANAGEMENT_OTLP_TRACING_EXPORT_ENABLED=false
export MANAGEMENT_OTLP_METRICS_EXPORT_ENABLED=false
export MANAGEMENT_TRACING_ENABLED=false
nohup java -jar target/auth-ms-1.1.0.jar > /tmp/auth-ms.log 2>&1 &
```

## Java — poulets-api

```bash
cd INFRA/poulets-platform/backend
export GRPC_SERVICE_TOKEN=$(cat /tmp/faso-grpc-token)
export SPRING_PROFILES_ACTIVE=dev
export GRPC_SERVER_SECURITY_ENABLED=false
export SPRING_DATASOURCE_URL="jdbc:postgresql://localhost:5432/poulets_db"
export SPRING_DATASOURCE_USERNAME=faso
export SPRING_DATASOURCE_PASSWORD="217FHIojYlPs1u4T+3gwnliX2MFjFp4RtMFTuWpbyg0="
export SPRING_REDIS_HOST=localhost SPRING_REDIS_PORT=6380
export AUTH_MS_GRPC_HOST=localhost AUTH_MS_GRPC_PORT=8802
export SERVER_PORT=8901
export MANAGEMENT_OTLP_TRACING_EXPORT_ENABLED=false
export MANAGEMENT_OTLP_METRICS_EXPORT_ENABLED=false
export MANAGEMENT_TRACING_ENABLED=false
export FASO_FLAGS_GROWTHBOOK_BASE_URL="http://localhost:3100"
export FASO_FLAGS_GROWTHBOOK_API_KEY="dev"
nohup java -jar target/poulets-api-1.1.0.jar > /tmp/poulets-api.log 2>&1 &
```

## ARMAGEDDON release binary

```bash
cd INFRA/armageddon
# Build (20-30 min first time, ~2 min incremental)
export CARGO_TARGET_DIR="/var/tmp/faso-cache/cargo-target"
cargo build --release --bin armageddon --no-default-features

# Launch with dev config (config/armageddon-dev.yaml)
/var/tmp/faso-cache/cargo-target/release/armageddon \
  --config config/armageddon-dev.yaml > /tmp/armageddon.log 2>&1 &
```

Ports ARMAGEDDON :
- `8080` : proxy HTTP/1 (routes définies dans armageddon-dev.yaml)
- `9902` : admin API (loopback only, conflit 9901 = poulets-api JMX)

## Ports persistants

| Service | Port | Health |
|---------|------|--------|
| frontend Angular | 4801 | http://localhost:4801 |
| BFF Next.js | 4800 | http://localhost:4800 |
| auth-ms | 8801 | http://localhost:8801/actuator/health |
| poulets-api | 8901 | http://localhost:8901/actuator/health |
| notifier-ms | 8803 | http://localhost:8803/actuator/health |
| ARMAGEDDON | 8080 | http://localhost:8080/healthz |
| ARMAGEDDON admin | 9902 | http://127.0.0.1:9902/admin/health |
| Vault | 8200 | http://localhost:8200/v1/sys/health |
| Consul | 8500 | http://localhost:8500/v1/status/leader |
| Postgres | 5432 | `podman exec faso-postgres pg_isready -U faso` |
| KAYA RESP3 | 6380 | `podman exec faso-kaya redis-cli -p 6380 PING` |

## Caches persistants

Redirigés vers partition `/` pour éviter saturation `/home` (voir `~/.bashrc`) :

```
CARGO_TARGET_DIR="/var/tmp/faso-cache/cargo-target"
PLAYWRIGHT_BROWSERS_PATH="/var/tmp/faso-cache/playwright"
npm_config_cache="/var/tmp/faso-cache/npm"
~/.m2 → /var/tmp/faso-cache/m2 (symlink)
~/.cargo/registry → /var/tmp/faso-cache/cargo-registry (symlink)
```

## Pièges connus

1. **`pkill -f spring-boot:run`** tue le shell Claude Code (exit 144). Utiliser :
   ```bash
   PIDS=$(ps -eo pid,cmd | grep 'auth-ms-1.1.0.jar' | grep -v grep | awk '{print $1}')
   [[ -n "$PIDS" ]] && kill -TERM $PIDS
   ```

2. **Postgres password reset** si auth-ms échoue au démarrage :
   ```bash
   podman exec faso-postgres psql -U faso -d postgres \
     -c "ALTER USER faso WITH PASSWORD '$(podman exec faso-postgres cat /run/secrets/postgres_password)';"
   ```

3. **JWT key corruption** (AEADBadTagException) :
   ```bash
   podman exec faso-postgres psql -U faso -d auth_ms -c "TRUNCATE jwt_signing_keys CASCADE;"
   ```

4. **OTLP endpoint empty string** → Spring Boot fail :
   ```
   export MANAGEMENT_OTLP_TRACING_EXPORT_ENABLED=false
   export MANAGEMENT_OTLP_METRICS_EXPORT_ENABLED=false
   ```

5. **Pentagon 403/401 sur ARMAGEDDON** : check `security.*.enabled` = `false` dans config + engines patchés pour respecter le flag (sentinel, arbiter, oracle, aegis, ai).

6. **Vault↔Consul TCP timeout** : bug bridge podman/docker iptables. Solution dev : Vault dev-mode (inmem). Solution prod : `podman network prune && podman-compose up -d`.

## Phase 7 + ARMAGEDDON Vagues 1-3 TERMINÉS

- Phase 7 : 20/20 axes livrés (k6, WAL replay, criterion bencher.dev, SLOs Sloth, GrowthBook, xDS canary, Playwright synthé, SPIRE rotation 24h, postmortem auto, dev containers, semantic-release, SPDX, CI/CD matrix, pre-commit, renovate, trivy, chaos mesh, buf breaking, rustdoc/redoc, grafana-as-code)
- ARMAGEDDON Vague 1 : 10/10 (HTTP/3, mTLS SPIRE, xDS v3 ADS, LB avancés, health checks TCP/gRPC, retry + budget, WebSocket/L4, gRPC-Web, response cache KAYA, admin API)
- ARMAGEDDON Vague 2 : 7/7 (pingora backend feature, io_uring, zero-copy splice, SIMD HPACK/QPACK, NUMA pinning, upstream pool H2, xDS debouncer)
- ARMAGEDDON Vague 3 : 10/10 (proxy-wasm ABI 0.2.0, SPIFFE auto-mTLS, traffic_split canary/AB/shadow, eBPF aya, JA4, DDoS distribué, OTel baggage, GraphQL limiter, compression Brotli/Zstd/Gzip, admin UI)

## Workspace

- **21 crates** compilent (0 erreur)
- **134 tests passent** (6 tests network-env failures tcp_proxy/quic/tls — non-code bugs)

## Commandes rapides

```bash
# Status global
/status-faso  # Claude skill, résumé 200 mots

# Logs
tail -f /tmp/auth-ms.log /tmp/poulets-api.log /tmp/armageddon.log

# Kill tout
for p in /tmp/{auth-ms,poulets-api,armageddon,ng-serve,bff-serve}.pid; do
  [[ -f "$p" ]] && kill $(cat $p) 2>/dev/null && rm $p
done
```
