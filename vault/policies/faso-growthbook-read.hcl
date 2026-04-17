# SPDX-License-Identifier: AGPL-3.0-or-later
path "faso/data/growthbook/*"     { capabilities = ["read"] }
path "faso/metadata/growthbook/*" { capabilities = ["list","read"] }
path "auth/token/renew-self"  { capabilities = ["update"] }
path "auth/token/lookup-self" { capabilities = ["read"] }
