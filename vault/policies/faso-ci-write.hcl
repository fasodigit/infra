# SPDX-License-Identifier: AGPL-3.0-or-later
# CI/CD (GitHub Actions OIDC) — rotate secrets + write deployment tokens.

path "faso/data/ci/*"     { capabilities = ["create","read","update","delete"] }
path "faso/metadata/ci/*" { capabilities = ["list","read","delete"] }

# Allow CI to rotate service secrets (but NOT read them).
path "faso/data/kaya/*"        { capabilities = ["update"] }
path "faso/data/armageddon/*"  { capabilities = ["update"] }
path "faso/data/auth-ms/*"     { capabilities = ["update"] }
path "faso/data/poulets-api/*" { capabilities = ["update"] }
path "faso/data/notifier-ms/*" { capabilities = ["update"] }

# Transit: rotate master keys (but not export).
path "transit/keys/*/rotate" { capabilities = ["update"] }

path "auth/token/renew-self"  { capabilities = ["update"] }
path "auth/token/lookup-self" { capabilities = ["read"] }
