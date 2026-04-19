<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# FASO DIGITALISATION — Vault + Consul

Stockage sécurisé centralisé des secrets pour toute l'infrastructure souveraine.

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                    Vault (2 replicas HA cible)                   │
│  KV v2 · Database · Transit · PKI intermediate · JWT/OIDC auth  │
└──────────────────────────┬───────────────────────────────────────┘
                           │ gossip + RPC
                           ▼
┌──────────────────────────────────────────────────────────────────┐
│              Consul (3 servers quorum, backend Vault)            │
│     storage · service discovery · ACL (jeton Vault dédié)        │
└──────────────────────────────────────────────────────────────────┘

Consommateurs Vault (auth : AppRole / Kubernetes Service Account) :
 ├─ KAYA                    → KV faso/kaya/*, Transit chiffrement PII
 ├─ ARMAGEDDON              → KV faso/armageddon/*, PKI mTLS upstream
 ├─ auth-ms (Java)          → KV faso/auth-ms/*, DB creds dynamic, Transit JWT sign
 ├─ poulets-api (Java)      → KV faso/poulets-api/*, DB creds dynamic
 ├─ notifier-ms (Java)      → KV faso/notifier-ms/*, SMTP creds
 ├─ BFF (Node)              → KV faso/bff/* (session cookie secrets)
 ├─ ORY Kratos              → KV faso/kratos/*, DB creds
 ├─ ORY Keto                → KV faso/keto/*, DB creds
 └─ CI (GitHub Actions OIDC)→ KV faso/ci/* (lecture seule, rotation 15 min)
```

## Namespaces KV (convention)

```
faso/
├── kaya/
│   ├── auth-password                   # RESP3 AUTH
│   ├── functions-signing-key           # HMAC-SHA256 FUNCTIONS
│   └── persistence-encryption-key      # TDE WAL (futur)
├── armageddon/
│   ├── admin-token                     # X-Admin-Token loopback API
│   ├── github-webhook-secret           # HMAC X-Hub-Signature-256
│   ├── redpanda-sasl-password          # consumer notifier-ms
│   └── rustls-server-cert-pem          # fallback non-SPIRE
├── auth-ms/
│   ├── jwt-encryption-key              # AES-256-GCM for privateKeyPem
│   ├── grpc-service-token              # service-to-service gRPC
│   └── brute-force-redis-password      # (placeholder — KAYA real)
├── poulets-api/
│   └── grpc-service-token
├── notifier-ms/
│   ├── smtp-username
│   ├── smtp-password
│   └── dedupe-redis-password
├── bff/
│   ├── session-cookie-secret
│   └── nextauth-secret
├── ory/
│   ├── kratos-cookie-secret
│   ├── kratos-cipher-secret
│   └── keto-secret
├── growthbook/
│   ├── jwt-secret
│   └── encryption-key
└── ci/
    └── (populated at runtime via GitHub OIDC, no static secrets)
```

## Démarrage

```bash
cd INFRA/docker/compose
# 1. Démarrer Consul + Vault (en plus des services existants)
podman-compose -f podman-compose.yml -f ../../vault/podman-compose.vault.yml up -d consul vault

# 2. Initialiser Vault (5 unseal keys, threshold 3) + policies + engines
bash ../../vault/scripts/init.sh
# → Écrit ~/.faso-vault-keys.json (chmod 600, .gitignored globalement)

# 3. Seed les secrets depuis les fichiers docker secrets existants
export VAULT_ADDR=http://127.0.0.1:8200
export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
bash ../../vault/scripts/seed-secrets.sh

# 4. Vérifier
vault kv get faso/kaya/auth-password
vault policy list
vault audit list
```

## Auth methods activés

| Méthode | Usage |
|---------|-------|
| AppRole | Services Rust/Java en dev local (role_id+secret_id injectés dans .env) |
| Kubernetes | Services en production (SA token → Vault auth) |
| JWT/OIDC | GitHub Actions (OIDC ID token → Vault → scope `faso/ci/*`) |
| token (root) | Admin bootstrap uniquement ; **révoquer après init.sh** |

## Policies (principe du moindre privilège)

Voir `policies/*.hcl`. Chaque service a sa policy nommée `faso-<service>-read`
avec accès uniquement à son sous-arbre KV. Les permissions de rotation (CI)
sont séparées (`faso-ci-write`).

## Secrets dynamiques PostgreSQL

Les microservices Java utilisent `database/creds/<role>` au lieu d'une
password statique :

```yaml
# application.yml
spring:
  datasource:
    url: jdbc:postgresql://postgres:5432/auth_ms
    username: ${DB_USER}      # injecté par Spring Cloud Vault
    password: ${DB_PASSWORD}  # TTL 1h, rotation auto
  cloud:
    vault:
      uri: http://vault:8200
      authentication: APPROLE
      app-role:
        role-id: ${VAULT_ROLE_ID}
        secret-id: ${VAULT_SECRET_ID}
      database:
        enabled: true
        role: auth-ms-readwrite
```

## Transit engine (encrypt-as-a-service)

`auth-ms` utilise Transit pour protéger les `private_key_pem` des JWT
signing keys (cf. `EncryptedStringConverter` JPA) :

```bash
vault write transit/encrypt/jwt-key plaintext=$(echo "-----BEGIN EC PRIVATE KEY..." | base64)
# → ciphertext vault:v1:…
vault write transit/decrypt/jwt-key ciphertext=vault:v1:…
# → plaintext base64
```

Avantage : rotation de la clé de chiffrement maîtresse sans toucher la base.

## PKI intermediate (upstream SPIRE)

Vault PKI fournit la **Root CA** pour `spiffe://faso.gov.bf/`.
SPIRE server se configure avec `UpstreamAuthority "vault"` pour obtenir
un certificat intermédiaire, puis signe les SVIDs workloads.

Voir `vault/scripts/configure-pki.sh`.

## Audit log

Activé vers `/var/log/vault/audit.log` + socket → Loki via Promtail. Tout
accès en lecture/écriture est loggé avec hash HMAC des tokens (pas les
tokens bruts).

## Backup / Restore

```bash
# Snapshot Consul (Vault uses Consul backend)
podman exec faso-consul consul snapshot save /consul/backups/$(date +%Y%m%d-%H%M).snap

# Restore
podman exec faso-consul consul snapshot restore /consul/backups/XXXX.snap
```

## Sécurité production (checklist)

- [ ] Vault TLS en production (cert via PKI FASO, pas auto-signé dev)
- [ ] Consul gossip encryption key : `consul keygen`
- [ ] Auto-unseal via Transit d'un autre cluster Vault, ou HSM (dev : manual unseal OK)
- [ ] Audit log vers stockage WORM (immuable)
- [ ] Rotation root token **après** bootstrap
- [ ] AppRole secret_id response-wrapped, TTL 24 h
- [ ] ACLs Consul activées (pas dev mode)
- [ ] Network policies : seul Vault peut accéder au port Consul server
- [ ] Sceller Vault à chaque redéploiement (principe de précaution)

## Ressources

- Documentation Vault : https://developer.hashicorp.com/vault/docs
- Documentation Consul : https://developer.hashicorp.com/consul/docs
- Sovereignty rule : Vault et Consul **restent** HashiCorp (pas de remplacement
  Rust à ce stade). Alternative long-terme envisagée : `openbao` (fork
  open-source de Vault) lorsque stable.
