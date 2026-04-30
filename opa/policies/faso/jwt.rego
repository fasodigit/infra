# SPDX-License-Identifier: AGPL-3.0-or-later
package faso.jwt

import future.keywords.if
import future.keywords.in

# ──────────────────────────────────────────────────────────────────────
# Extract & verify the JWT from input headers.
# ARMAGEDDON has already verified the signature in its `jwt` filter and
# forwards the raw token AND its parsed claims as input attributes.
# We re-extract the claims here for policy decisions.
# ──────────────────────────────────────────────────────────────────────

# Payload is forwarded by ARMAGEDDON already-parsed (signature verified by
# the gateway's own jwt filter). The ext_authz contract REQUIRES the gateway
# to set `input.jwt_payload` after JWT validation; if absent, payload is
# undefined and downstream rules silently fail (resulting in deny).
payload := p if {
    p := input.jwt_payload
}

# Public-route helpers
is_public if {
    input.path in [
        "/", "/auth/login", "/auth/register", "/auth/forgot-password",
        "/health", "/metrics", "/.well-known/jwks.json",
    ]
}
is_public if startswith(input.path, "/auth/")
is_public if startswith(input.path, "/identity/")

# Authenticated user id (sub claim)
user_id := uid if {
    uid := payload.sub
    uid != ""
}

# Roles claim (Kratos identity traits.roles → JWT `roles` claim)
roles := r if {
    r := payload.roles
} else := [] if true

has_role(role) if role in roles
