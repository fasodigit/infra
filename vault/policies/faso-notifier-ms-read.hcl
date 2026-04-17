# SPDX-License-Identifier: AGPL-3.0-or-later
path "faso/data/notifier-ms/*"     { capabilities = ["read"] }
path "faso/metadata/notifier-ms/*" { capabilities = ["list","read"] }
path "database/creds/notifier-ms-readwrite" { capabilities = ["read"] }
path "auth/token/renew-self"  { capabilities = ["update"] }
path "auth/token/lookup-self" { capabilities = ["read"] }
