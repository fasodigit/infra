# SPDX-License-Identifier: AGPL-3.0-or-later
# ARMAGEDDON gateway — secret read + PKI sign for non-SPIRE fallback + transit for PII mask.

path "faso/data/armageddon/*"     { capabilities = ["read"] }
path "faso/metadata/armageddon/*" { capabilities = ["list","read"] }

# PKI: sign intermediate certificates for gateway identity (when SPIRE is unavailable).
path "pki/issue/armageddon" { capabilities = ["update"] }
path "pki/sign/armageddon"  { capabilities = ["update"] }

# Transit: encrypt PII in response bodies (VEIL component).
path "transit/encrypt/pii-key" { capabilities = ["update"] }
path "transit/decrypt/pii-key" { capabilities = ["update"] }

path "auth/token/renew-self"  { capabilities = ["update"] }
path "auth/token/lookup-self" { capabilities = ["read"] }
