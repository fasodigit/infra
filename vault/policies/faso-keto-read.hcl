# SPDX-License-Identifier: AGPL-3.0-or-later
path "faso/data/ory/keto"  { capabilities = ["read"] }
path "faso/data/postgres"  { capabilities = ["read"] }
path "database/creds/keto-readwrite" { capabilities = ["read"] }
path "auth/token/renew-self"  { capabilities = ["update"] }
path "auth/token/lookup-self" { capabilities = ["read"] }
