# KAYA Deployment Guide

KAYA is the sovereign in-memory database (RESP3-compatible) replacing Redis in
the FASO DIGITALISATION ecosystem. This document covers local builds, container
builds, runtime configuration, persistence, and security.

---

## 1. Build manuel (développement local)

### Prérequis

- Rust 1.85+ (`rustup update stable`)
- `pkg-config`, `libssl-dev`, `cmake` (Ubuntu/Debian: `apt install pkg-config libssl-dev cmake`)

### Compiler

```bash
cd INFRA/kaya

# Release (optimisé, LTO thin, strip)
cargo build --release -p kaya-server -p kaya-cli

# Binaires produits
ls -lh target/release/kaya-server   # ~13 MB stripped
ls -lh target/release/kaya-cli      # ~1.1 MB stripped
```

### Smoke test local

```bash
# Démarrer le serveur (port 6380 par défaut)
./target/release/kaya-server --config config/default.yaml &

# Vérifier
./target/release/kaya-cli ping          # PONG
./target/release/kaya-cli SET foo bar   # OK
./target/release/kaya-cli GET foo       # bar

kill %1
```

---

## 2. Build Docker / Podman

### Construire l'image

Build depuis la racine `INFRA/` (le contexte inclut `kaya/` et `docker/`):

```bash
cd /home/lyna/Documents/DEVELOPMENT-CLAUDE/INFRA

# Via podman (recommandé — rootless, sans daemon)
podman build \
  -f docker/images/Containerfile.kaya \
  -t faso/kaya:dev \
  .

# Vérifier la taille de l'image
podman images faso/kaya:dev
```

L'image cible `debian:bookworm-slim` avec uniquement `ca-certificates`.
Taille attendue : 50-80 MB.

### Via podman-compose (méthode recommandée en CI/CD)

```bash
cd INFRA/docker/compose
podman-compose -f podman-compose.yml build kaya
```

---

## 3. Démarrage via podman-compose

```bash
cd INFRA/docker/compose

# Démarrer KAYA seul
podman-compose -f podman-compose.yml up -d kaya

# Vérifier le statut
podman-compose -f podman-compose.yml ps kaya

# Logs en temps réel
podman-compose -f podman-compose.yml logs -f kaya

# Smoke test dans le container
podman exec faso-kaya /usr/local/bin/kaya-cli --port 6379 ping
```

### Ports exposés (loopback uniquement en production)

| Port hôte | Port container | Protocole |
|-----------|----------------|-----------|
| 6380      | 6379           | RESP3 (Redis-compatible) |
| 6381      | 6381           | gRPC API |
| 9100      | 9100           | Prometheus metrics |

---

## 4. Configuration dev vs prod

### Dev (config/default.yaml — valeurs actuelles)

```yaml
server:
  resp_port: 6380      # 6379 en container via --port arg
  bind: "0.0.0.0"
  max_connections: 10000

store:
  num_shards: 64
  eviction_policy: "lru"
  max_memory: 0        # illimité en dev

persistence:
  enabled: false       # WAL désactivé en dev

observe:
  metrics_enabled: true
  metrics_port: 9100
  log_level: "info"
```

### Prod (overrides recommandés via env ou fichier config séparé)

```yaml
store:
  num_shards: 128      # adapter au nombre de cœurs CPU
  max_memory: 4294967296   # 4 GB max

persistence:
  enabled: true
  data_dir: "/var/lib/kaya"
  fsync_policy: "always"   # durabilité maximale (+ latence ~5-20ms)
  segment_size_bytes: 134217728   # 128 MiB par segment WAL
  snapshot_interval_secs: 3600
  snapshot_retention: 7
  compression: "zstd"
  zstd_level: 3

security:
  acl_enabled: true
  default_password: ""   # via KAYA_PASSWORD env (cf. section Sécurité)

observe:
  json_logging: true
  tracing_enabled: true
  otlp_endpoint: "http://jaeger:4317"
```

### Fsync policies

| Policy      | Durabilité | Latence ajoutée | Cas d'usage |
|-------------|-----------|-----------------|-------------|
| `always`    | maximale  | +5-20 ms/op     | prod financier, ACIDe |
| `every_sec` | 1s window | +0.5-2 ms       | prod standard (défaut) |
| `no`        | OS-managed | 0               | cache pur, perte acceptable |

---

## 5. Persistance : volume kaya-data

Le volume `faso-kaya-data` est monté sur `/var/lib/kaya` dans le container.
Il stocke :
- `wal/` — segments Write-Ahead Log (rotation à 64 MiB par défaut)
- `snapshots/` — instantanés compressés Zstd par shard

```bash
# Créer le volume explicitement (optionnel, podman-compose le crée auto)
podman volume create faso-kaya-data

# Inspecter
podman volume inspect faso-kaya-data

# En production, mapper un répertoire NVMe dédié
# Dans podman-compose.yml, remplacer :
#   kaya-data:
#     driver: local
#     name: faso-kaya-data
# par :
#   kaya-data:
#     driver: local
#     name: faso-kaya-data
#     driver_opts:
#       type: none
#       o: bind
#       device: /mnt/nvme0/kaya-data
```

---

## 6. Sécurité : AUTH via Vault KV

Le mot de passe KAYA est stocké dans Vault sous `faso/kaya/auth`.

### Seed Vault (une fois)

```bash
export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
vault kv put secret/faso/kaya/auth password="$(openssl rand -base64 32)"
```

### Injecter dans le container

Deux méthodes selon l'environnement :

**A. Variable d'environnement (podman-compose dev)**

```yaml
# Dans podman-compose.yml, section kaya > environment :
environment:
  KAYA_PASSWORD: "${KAYA_AUTH_PASSWORD}"   # depuis .env
```

Le serveur lit `--password` via `Args.password`. Passer via CMD :
```
CMD ["--config", "/etc/kaya/default.yaml", "--port", "6379", "--bind", "0.0.0.0", "--password", "${KAYA_PASSWORD}"]
```

**B. Vault Agent (prod)**

Vault Agent injecte le secret dans un fichier monté :
```
/run/secrets/kaya_password
```

Et le server est lancé avec :
```bash
kaya-server --password "$(cat /run/secrets/kaya_password)"
```

### Configuration ACL (prod)

```yaml
security:
  acl_enabled: true
  acl_file: "/etc/kaya/acl.conf"
```

Exemple `acl.conf` :
```
# user <nom> on/off ~<key-pattern> +<commands>
user default off nopass
user poulets-api on >strongpassword ~poulets:* +@all
user auth-ms on >strongpassword ~sessions:* +@all
user readonly on >readonlypass ~* +@read
```

---

## 7. Vérification santé post-démarrage

```bash
# Health check RESP3
podman exec faso-kaya /usr/local/bin/kaya-cli --port 6379 ping

# Métriques Prometheus
curl -s http://localhost:9100/metrics | grep kaya_

# Test complet SET/GET/GEO/PUBSUB
podman exec faso-kaya /usr/local/bin/kaya-cli --port 6379 SET foo bar
podman exec faso-kaya /usr/local/bin/kaya-cli --port 6379 GET foo
podman exec faso-kaya /usr/local/bin/kaya-cli --port 6379 GEOADD cities 2.35 48.85 Paris
podman exec faso-kaya /usr/local/bin/kaya-cli --port 6379 GEOSEARCH cities FROMMEMBER Paris BYRADIUS 1000 km ASC
```
