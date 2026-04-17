# SPDX-License-Identifier: AGPL-3.0-or-later
path "faso/data/poulets-api/*"     { capabilities = ["read"] }
path "faso/metadata/poulets-api/*" { capabilities = ["list","read"] }
path "database/creds/poulets-api-readwrite" { capabilities = ["read"] }
path "auth/token/renew-self"  { capabilities = ["update"] }
path "auth/token/lookup-self" { capabilities = ["read"] }
