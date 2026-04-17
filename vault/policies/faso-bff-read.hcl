# SPDX-License-Identifier: AGPL-3.0-or-later
path "faso/data/bff/*"     { capabilities = ["read"] }
path "faso/metadata/bff/*" { capabilities = ["list","read"] }
path "faso/data/ory/kratos" { capabilities = ["read"] }
path "auth/token/renew-self"  { capabilities = ["update"] }
path "auth/token/lookup-self" { capabilities = ["read"] }
