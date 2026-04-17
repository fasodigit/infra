# SPDX-License-Identifier: AGPL-3.0-or-later
# auth-ms — secret read + dynamic DB creds + transit encrypt JWT signing keys.

path "faso/data/auth-ms/*"     { capabilities = ["read"] }
path "faso/metadata/auth-ms/*" { capabilities = ["list","read"] }

# Dynamic PostgreSQL credentials (TTL 1h, rotated automatically).
path "database/creds/auth-ms-readwrite" { capabilities = ["read"] }

# Transit: envelope-encrypt the JWT private keys at rest (EncryptedStringConverter JPA).
path "transit/encrypt/jwt-key" { capabilities = ["update"] }
path "transit/decrypt/jwt-key" { capabilities = ["update"] }

# KV read-only for shared ORY config (Kratos JWK endpoint).
path "faso/data/ory/kratos" { capabilities = ["read"] }

path "auth/token/renew-self"  { capabilities = ["update"] }
path "auth/token/lookup-self" { capabilities = ["read"] }
