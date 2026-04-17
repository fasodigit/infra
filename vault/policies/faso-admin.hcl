# SPDX-License-Identifier: AGPL-3.0-or-later
# Admin policy — bootstrap + operational ops. Restrict with additional MFA in production.

path "sys/policies/*" { capabilities = ["create","read","update","delete","list"] }
path "sys/mounts*"    { capabilities = ["create","read","update","delete","sudo"] }
path "sys/auth/*"     { capabilities = ["create","read","update","delete","sudo"] }
path "sys/audit/*"    { capabilities = ["create","read","update","delete","sudo"] }
path "sys/health"     { capabilities = ["read"] }
path "sys/capabilities-self" { capabilities = ["update"] }

path "auth/token/*"   { capabilities = ["create","read","update","delete","list","sudo"] }
path "auth/approle/*" { capabilities = ["create","read","update","delete","list"] }

path "faso/*"         { capabilities = ["create","read","update","delete","list"] }
path "database/*"     { capabilities = ["create","read","update","delete","list"] }
path "transit/*"      { capabilities = ["create","read","update","delete","list"] }
path "pki/*"          { capabilities = ["create","read","update","delete","list","sudo"] }
