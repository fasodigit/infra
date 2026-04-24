# PII Encryption at Rest — Implementation Guide

SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
SPDX-License-Identifier: AGPL-3.0-or-later

## Overview

`PiiEncryptionConverter` provides transparent AES-256-GCM encryption for
PII (Personally Identifiable Information) columns via JPA `@Convert`.
This satisfies Loi 010-2004 data protection requirements and aligns with
the existing `EncryptedStringConverter` pattern used for JWT signing keys
in auth-ms.

## Entity Fields Requiring Encryption

### auth-ms — `User` entity (`auth-ms/src/main/java/.../auth/model/User.java`)

| Field | Column | PII Type |
|-------|--------|----------|
| `email` | `email` | Contact / identity |
| `firstName` | `first_name` | Identity |
| `lastName` | `last_name` | Identity |
| `phoneNumber` | `phone_number` | Contact |

### poulets-platform — `Client` entity (`poulets-platform/backend/src/main/java/.../poulets/model/Client.java`)

| Field | Column | PII Type |
|-------|--------|----------|
| `name` | `name` | Identity |
| `phone` | `phone` | Contact |
| `address` | `address` | Location |

### poulets-platform — `Eleveur` entity (`poulets-platform/backend/src/main/java/.../poulets/model/Eleveur.java`)

| Field | Column | PII Type |
|-------|--------|----------|
| `name` | `name` | Identity |
| `phone` | `phone` | Contact |
| `location` | `location` | Location |

### notifier-ms — `NotificationDelivery` entity (`notifier-ms/notifier-core/src/main/java/.../notifier/domain/NotificationDelivery.java`)

| Field | Column | PII Type |
|-------|--------|----------|
| `recipient` | `recipient` | Contact (email) |

## How to Apply

Add the `@Convert` annotation to each PII field:

```java
import bf.gov.faso.audit.crypto.PiiEncryptionConverter;
import jakarta.persistence.Convert;

@Convert(converter = PiiEncryptionConverter.class)
@Column(nullable = false)
private String email;
```

## Key Provisioning

1. **Generate key:**
   ```bash
   openssl rand -base64 32
   ```

2. **Store in Vault:**
   ```bash
   vault kv put faso/shared/pii-encryption-key \
     value="$(openssl rand -base64 32)"
   ```

3. **Inject as environment variable:**
   ```bash
   export FASO_PII_ENCRYPTION_KEY=$(vault kv get -field=value faso/shared/pii-encryption-key)
   ```

4. **In podman-compose**, reference via Vault Agent or `.env`:
   ```yaml
   environment:
     FASO_PII_ENCRYPTION_KEY: ${FASO_PII_ENCRYPTION_KEY}
   ```

## Migration Strategy (Encrypt Existing Data)

Encryption must be applied in a **coordinated migration** to avoid
breaking running queries:

### Phase 1: Dual-Read (backward compatible)

1. Add a new encrypted column (e.g., `email_enc`) alongside the
   plaintext column.
2. Deploy a Flyway migration that copies and encrypts existing data:
   ```sql
   -- Application-level migration (use a Spring Boot CommandLineRunner)
   -- SQL cannot call AES-GCM directly; use a batch Java job instead.
   ```
3. Write to both columns; read from encrypted column first, fall back
   to plaintext.

### Phase 2: Cutover

1. Verify all rows have encrypted values.
2. Switch reads to encrypted-only.
3. Drop plaintext column via a follow-up Flyway migration.
4. Rename `email_enc` → `email`.

### Phase 3: Verify

1. Run integration tests confirming round-trip encrypt/decrypt.
2. Verify no plaintext PII remains in database dumps.

## Searching Encrypted Fields (Hash Index Approach)

Since AES-GCM ciphertext is non-deterministic (random IV), you cannot
use `WHERE email = ?` on encrypted columns. Two approaches:

### Option A: Blind Index (recommended)

Store a HMAC-SHA256 hash alongside the encrypted value:

```sql
ALTER TABLE users ADD COLUMN email_hash BYTEA;
CREATE UNIQUE INDEX idx_users_email_hash ON users (email_hash);
```

```java
// At write time:
String hash = hmacSha256(email, FASO_PII_SEARCH_KEY);
user.setEmailHash(hash);
user.setEmail(email); // encrypted by converter

// At query time:
String hash = hmacSha256(searchEmail, FASO_PII_SEARCH_KEY);
User user = repo.findByEmailHash(hash);
```

This requires a **separate** HMAC key (not the AES key) stored in Vault
at `faso/shared/pii-search-key`.

### Option B: Application-level filtering

For low-cardinality queries, decrypt in the application layer:
```java
List<User> all = repo.findAll();
return all.stream()
    .filter(u -> u.getEmail().equals(searchEmail))
    .toList();
```

This is acceptable only for admin queries on small datasets.

## Key Rotation

1. Generate new key in Vault.
2. Deploy batch job that re-encrypts all PII columns with new key.
3. Update `FASO_PII_ENCRYPTION_KEY` env var.
4. Redeploy services.
5. Archive old key (retain for 5 years per Loi 010-2004).
