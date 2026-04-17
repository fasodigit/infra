# SPDX-License-Identifier: AGPL-3.0-or-later
# KAYA in-memory DB — secret read + transit usage for persistence encryption.

path "faso/data/kaya/*"       { capabilities = ["read"] }
path "faso/metadata/kaya/*"   { capabilities = ["list","read"] }

# Transit key for WAL / snapshot encryption (encrypt + decrypt, NOT export).
path "transit/encrypt/persistence-key" { capabilities = ["update"] }
path "transit/decrypt/persistence-key" { capabilities = ["update"] }

# Token self-management.
path "auth/token/renew-self" { capabilities = ["update"] }
path "auth/token/lookup-self" { capabilities = ["read"] }
