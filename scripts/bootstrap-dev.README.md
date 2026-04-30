<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# `bootstrap-dev.sh` — Vault migration path & operator guide

This document explains:

1. What `bootstrap-dev.sh` does and how to invoke it.
2. **The mapping between every dev secret and its production Vault path** (KV v2
   under `faso/<service>/<usage>` per `INFRA/CLAUDE.md` §2).
3. How each Java service is wired (or should be wired) to read its secrets
   from Vault via Spring Cloud Vault.
4. The `JWT_KEY_ENCRYPTION_KEY` rotation strategy (currently destructive — a
   dual-version migration plan is documented below).

> **Source of truth for sovereignty / podman-compose / Vault**: `INFRA/CLAUDE.md`.

---

## 1. Quickstart

```bash
# Idempotent converge — safe to re-run any number of times
bash INFRA/scripts/bootstrap-dev.sh

# Print the plan without executing
bash INFRA/scripts/bootstrap-dev.sh --dry-run

# DESTRUCTIVE: drops auth_ms + poulets_db + notifier, wipes all dev data
bash INFRA/scripts/bootstrap-dev.sh --reset
```

Acceptance: running the script twice in a row should print `✅ already
healthy` for each step. The first run converges. The second run is a no-op.

The script handles the 8 manual hacks documented in the
2026-04-29 RUNBOOK (`INFRA/RUNBOOK-LOCAL-STACK.md` — header of `bootstrap-dev.sh`):

| #  | Failure mode                                              | Step in script |
| -- | --------------------------------------------------------- | -------------- |
| 1  | Corrupt WAL → restart loop (run `pg_resetwal`)            | 2              |
| 2  | Drift in `pg_namespace` → recreate dev DBs                | 3 (idempotent) |
| 3  | DB user passwords drifted from compose secret             | 4              |
| 4  | `JWT_KEY_ENCRYPTION_KEY` env missing → auth-ms boot fails | 5              |
| 5  | Stale `jwt_signing_keys` rows from a previous JWT key     | 6              |
| 6  | Audit-lib BlindIndexConverter test missing PII keys       | 5              |
| 7  | `faso-consul` exited → Spring Cloud Consul refuses boot   | 7              |
| 8  | `CREATE SCHEMA audit` race between auth-ms + poulets-api  | 8              |

---

## 2. Vault path mapping (dev → prod)

All paths below are **KV v2 reads** (HTTP path: `${VAULT_ADDR}/v1/faso/data/<path>`)
unless explicitly noted as *DB engine static-creds*.

### Database credentials (HashiCorp Vault DB engine — NOT KV)

These are issued by the database secrets engine, **not** stored in KV. They
rotate automatically (TTL ≈ 1h, renew until 24h max). The runtime role grants
DML only; Flyway has its own DDL role.

| Service       | Dev source                               | Prod Vault role / path                                | Wiring                                                     |
| ------------- | ---------------------------------------- | ----------------------------------------------------- | ---------------------------------------------------------- |
| auth-ms       | `secrets/postgres_password.txt`          | `database/creds/auth-ms-runtime-role`                 | `application-vault.yml` → `spring.cloud.vault.database`    |
| auth-ms Flyway| same                                     | `database/creds/auth-ms-flyway-role`                  | `FASO_FLYWAY_USER` / `FASO_FLYWAY_PASSWORD` env injection  |
| poulets-api   | `secrets/postgres_password.txt`          | `database/creds/poulets-api-runtime-role`             | `application-vault.yml` → `spring.cloud.vault.database`    |
| poulets-api Flyway| same                                 | `database/creds/poulets-api-flyway-role`              | same env injection pattern                                 |
| notifier-ms   | `secrets/postgres_password.txt`          | `database/creds/notifier-ms-runtime-role`             | needs `application-vault.yml` (currently dev-only); add `spring.cloud.vault.database` block |

Configure these roles via `INFRA/vault/scripts/configure-database.sh`
(already exists). The runtime role grants `SELECT, INSERT, UPDATE, DELETE` on
service-owned schemas only; the flyway role adds `CREATE, ALTER, DROP`.

### KV v2 secrets — `faso/<service>/<usage>` paths

| Dev env var / file                                  | Vault KV path                       | Vault key            | Service consumer        | Notes                                                                  |
| --------------------------------------------------- | ----------------------------------- | -------------------- | ----------------------- | ---------------------------------------------------------------------- |
| `JWT_KEY_ENCRYPTION_KEY` (file `secrets/jwt-key-encryption-key.txt`) | `faso/auth-ms/jwt`         | `encryption_key_b64` | auth-ms                 | Already seeded by `INFRA/vault/scripts/seed-secrets.sh` line 60        |
| `FASO_PII_ENCRYPTION_KEY` (`/tmp/faso-pii-encryption-key`)            | `faso/poulets-api/pii`     | `encryption_key`     | poulets-api (audit-lib) | Used by `PiiEncryptionConverter`                                       |
| `FASO_PII_BLIND_INDEX_KEY` (`/tmp/faso-pii-blind-index-key`)          | `faso/poulets-api/pii`     | `blind_index_key`    | poulets-api (audit-lib) | Used by `BlindIndexConverter`                                          |
| (auth-ms also encrypts PII — same lib)              | `faso/auth-ms/pii`                  | `encryption_key`     | auth-ms                 | Mirror of poulets-api path, distinct key per service (blast radius)    |
| (auth-ms also has blind-index)                      | `faso/auth-ms/pii`                  | `blind_index_key`    | auth-ms                 | Mirror                                                                 |
| `GRPC_SERVICE_TOKEN` (auth-ms)                      | `faso/auth-ms/grpc`                 | `service_token`      | auth-ms                 | Already in `seed-secrets.sh` line 61                                   |
| `GRPC_SERVICE_TOKEN` (poulets-api)                  | `faso/poulets-api/grpc`             | `service_token`      | poulets-api             | Already seeded                                                         |
| `KAYA_PASSWORD`                                     | `faso/kaya/auth`                    | `password`           | all Java services       | Already seeded                                                         |
| `KETO_SECRET`                                       | `faso/ory/keto`                     | `secret`             | keto                    | Already seeded                                                         |
| `KRATOS_COOKIE_SECRET`                              | `faso/ory/kratos`                   | `cookie_secret`      | kratos                  | Already seeded                                                         |
| `KRATOS_CIPHER_SECRET`                              | `faso/ory/kratos`                   | `cipher_secret`      | kratos                  | Already seeded                                                         |
| `ARMAGEDDON_ADMIN_TOKEN`                            | `faso/armageddon/admin`             | `token`              | armageddon              | Already seeded                                                         |
| `GITHUB_WEBHOOK_SECRET`                             | `faso/armageddon/github`            | `webhook_secret`     | armageddon              | Already seeded                                                         |
| `BFF_SESSION_COOKIE_SECRET`                         | `faso/bff/session`                  | `cookie_secret`      | bff                     | Already seeded                                                         |
| `NEXTAUTH_SECRET`                                   | `faso/bff/nextauth`                 | `secret`             | bff                     | Already seeded                                                         |
| `SLACK_WEBHOOK_URL`                                 | `faso/notifier/slack`               | `webhook_url`        | notifier-ms             | Optional (high-priority security alerts only)                          |
| `MAILERSEND_SMTP_USER` / `MAILERSEND_SMTP_PASSWORD` | `faso/notifier/smtp`                | `username` / `password` | notifier-ms          | Already seeded                                                         |
| `GROWTHBOOK_JWT_SECRET`                             | `faso/growthbook`                   | `jwt_secret`         | growthbook              | Already seeded                                                         |
| `GROWTHBOOK_ENCRYPTION_KEY`                         | `faso/growthbook`                   | `encryption_key`     | growthbook              | Already seeded                                                         |

### Transit engine (recommended for cryptographic operations)

Better than KV: the secret never leaves Vault. Use `transit/encrypt/<key>` and
`transit/decrypt/<key>`. Already pre-created by `INFRA/vault/scripts/init.sh`:

* `transit/keys/jwt-key`           → auth-ms could use this *instead* of the local AES-GCM converter (eliminates `JWT_KEY_ENCRYPTION_KEY` rotation pain — see §4 below)
* `transit/keys/pii-key`           → audit-lib PII (encrypt at rest in DB)
* `transit/keys/persistence-key`   → KAYA WAL encryption

---

## 3. Spring Cloud Vault wiring per Java service

All three services already have an `application-vault.yml` profile; activate
with `--spring.profiles.active=vault` and inject `VAULT_ROLE_ID` +
`VAULT_SECRET_ID` (issued by `vault write -f auth/approle/role/faso-<svc>/secret-id`).

### auth-ms — `INFRA/auth-ms/src/main/resources/application-vault.yml`

```yaml
spring:
  cloud:
    vault:
      kv:
        backend: faso
        default-context: auth-ms     # reads faso/auth-ms/* into Spring env
      database:
        enabled: true
        role: auth-ms-runtime-role   # ← matches database/creds/auth-ms-runtime-role
        backend: database
```

To consume `faso/auth-ms/jwt.encryption_key_b64` Spring Cloud Vault flattens it
into `jwt.encryption_key_b64`. Bind via `@Value("${jwt.encryption_key_b64}")`
(currently `EncryptedStringConverter` reads `JWT_KEY_ENCRYPTION_KEY` — change
to `jwt.encryption_key_b64` or add an alias).

### poulets-api — `INFRA/poulets-platform/backend/src/main/resources/application-vault.yml`

Same pattern with `default-context: poulets-api`. Add bindings:

```java
@Value("${pii.encryption_key}")           String piiKey;
@Value("${pii.blind_index_key}")          String blindIdxKey;
```

(Currently `PiiEncryptionConverter` reads env `FASO_PII_ENCRYPTION_KEY` — wire
through Spring config instead so Vault rotation works.)

### notifier-ms — `INFRA/notifier-ms/notifier-core/src/main/resources/application-vault.yml`

Currently active for SMTP only. Extend with:

```yaml
spring:
  cloud:
    vault:
      kv:
        backend: faso
        default-context: notifier-ms
      database:
        enabled: true
        role: notifier-ms-runtime-role
        backend: database
```

---

## 4. `JWT_KEY_ENCRYPTION_KEY` rotation strategy

**Current behaviour (BROKEN for rotation):**

`auth-ms` `EncryptedStringConverter` AES-256-GCM-encrypts each
`jwt_signing_keys.private_key` row with the single env-injected key. Rotating
the key by setting a new `JWT_KEY_ENCRYPTION_KEY` makes every existing row
undecryptable → `JwtService.@PostConstruct` throws and auth-ms refuses to boot.
This is exactly what triggered hack #5 (manual `DELETE FROM jwt_signing_keys`).

**Migration plan (target — out of scope for `bootstrap-dev.sh`):**

1. Schema change: add `jwt_signing_keys.key_version SMALLINT NOT NULL DEFAULT 1`
   (Flyway `V4__multi_version_jwt_keys.sql`).
2. Config: replace the single `JWT_KEY_ENCRYPTION_KEY` with a versioned map:
   ```yaml
   jwt:
     encryption-keys:
       1: ${JWT_KEY_V1}
       2: ${JWT_KEY_V2}        # new key during rotation window
     active-version: 2
   ```
3. `EncryptedStringConverter` reads `key_version` from the row, picks the
   matching key, decrypts. New writes always use `active-version`.
4. Background `KeyRotationMigrationService` (already present as a stub —
   wire to actually run): scan rows where `key_version != active-version`,
   decrypt with old key, re-encrypt with new key, update version.
5. Once 100% of rows are at the new version, drop the old key from the map.

**Even better (long-term):** delegate to Vault Transit. Then the key never
leaves Vault, rotation is a single `vault write -f transit/keys/jwt-key/rotate`,
and Vault keeps old key versions automatically for decryption.

---

## 5. Idempotency contract

`bootstrap-dev.sh` is idempotent by design:

* Step 1: `init-secrets.sh` already skips existing files.
* Step 2: `pg_resetwal` runs **only** when `ctr_state == RESTARTING` AND logs
  contain `PANIC.*could not locate a valid checkpoint record`. Healthy
  Postgres → no-op.
* Step 3: `CREATE DATABASE` / `CREATE ROLE` are gated by existence checks.
* Step 4: `ALTER USER ... PASSWORD` is intrinsically idempotent in PG.
* Step 5: `gen_keyfile` skips if file is non-empty.
* Step 6: compares stored fingerprint vs current — only deletes when changed.
* Step 7: `start` on a `RUNNING` container is a no-op in podman/docker.
* Step 8: detects existing `audit.audit_log` table → skips re-apply.

Running `bootstrap-dev.sh` 3 consecutive times must produce 3× identical
output (modulo timestamps). The CI in `INFRA/scripts/tests/` should pin this
behaviour with a future `bootstrap-dev.test.sh`.

---

*Last updated: 2026-04-29*
