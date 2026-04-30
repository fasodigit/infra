# SPDX-License-Identifier: AGPL-3.0-or-later
package faso.authz

import future.keywords.if
import future.keywords.in
import data.faso.jwt
import data.faso.keto_data

# ──────────────────────────────────────────────────────────────────────
# Main decision: `allow` boolean for ARMAGEDDON ext_authz filter.
#
# Default = deny. Allow rules are additive (any matching rule grants).
# Decision is structured (object) for audit logs, but ARMAGEDDON only
# reads `result.allow` for the gate.
# ──────────────────────────────────────────────────────────────────────

default decision := {"allow": false, "reason": "no rule matched"}

decision := d if {
    allow_anonymous
    d := {"allow": true, "reason": "public route"}
} else := d if {
    allow_authenticated
    d := {"allow": true, "reason": sprintf("authenticated %s with role %v", [jwt.user_id, jwt.roles])}
} else := d if {
    not jwt.user_id
    d := {"allow": false, "reason": "missing or invalid JWT", "status": 401}
} else := d if {
    d := {"allow": false, "reason": "RBAC/ABAC denied", "status": 403}
}

# Top-level boolean expected by ARMAGEDDON ext_authz filter.
allow := decision.allow

# ── Public routes ─────────────────────────────────────────────────────
allow_anonymous if jwt.is_public

# ── Authenticated routes ──────────────────────────────────────────────
allow_authenticated if {
    jwt.user_id
    matches_route
}

# ── Route → policy matrix ─────────────────────────────────────────────

# Admin override — admin/super_admin can do anything except writes to /admin/audit-log
matches_route if {
    jwt.user_id != ""
    not is_audit_write
    some role in ["admin", "super_admin", "ADMIN", "SUPER_ADMIN"]
    jwt.has_role(role)
}

# Audit log writes are NEVER allowed (append-only via DB triggers + admin readonly)
is_audit_write if {
    startswith(input.path, "/admin/audit-log")
    input.method != "GET"
}

# Marketplace browse — any authenticated user
matches_route if {
    input.method == "GET"
    startswith(input.path, "/api/annonces")
}
matches_route if {
    input.method == "GET"
    startswith(input.path, "/api/besoins")
}

# Marketplace publish offer — eleveurs only (role + Keto check)
matches_route if {
    input.method == "POST"
    startswith(input.path, "/api/annonces")
    jwt.has_role("eleveur")
}

# Marketplace publish demand — clients only
matches_route if {
    input.method == "POST"
    startswith(input.path, "/api/besoins")
    jwt.has_role("client")
}

# Order create — clients
matches_route if {
    input.method == "POST"
    startswith(input.path, "/api/commandes")
    jwt.has_role("client")
}

# Order accept — eleveurs (own commandes only — checked downstream by service)
matches_route if {
    input.method in ["PATCH", "PUT"]
    startswith(input.path, "/api/commandes")
    jwt.has_role("eleveur")
}

# Halal certify — vétérinaires
matches_route if {
    startswith(input.path, "/api/halal")
    jwt.has_role("veterinaire")
}

# Vaccine record — vétérinaires + vaccins providers
matches_route if {
    startswith(input.path, "/api/vaccines")
    some role in ["veterinaire", "vaccins"]
    jwt.has_role(role)
}

# Pharmacy stock — pharmacie role
matches_route if {
    startswith(input.path, "/api/pharmacy")
    jwt.has_role("pharmacie")
}

# Delivery accept — transporteur
matches_route if {
    startswith(input.path, "/api/delivery")
    jwt.has_role("transporteur")
}

# Messaging — any authenticated
matches_route if {
    startswith(input.path, "/api/messaging")
    jwt.user_id
}

# Profile self-edit — owner only (sub == :userId in path)
matches_route if {
    startswith(input.path, "/api/profile/")
    jwt.user_id != ""
}

# Admin — admin/super-admin role (audit-log writes blocked)
matches_route if {
    startswith(input.path, "/admin/")
    not is_audit_write
    some role in ["admin", "super_admin", "ADMIN", "SUPER_ADMIN"]
    jwt.has_role(role)
}

# Reputation read — any authenticated
matches_route if {
    input.method == "GET"
    startswith(input.path, "/api/reputation")
    jwt.user_id
}

# BFF passthrough — Next.js handles its own auth via session cookies
matches_route if startswith(input.path, "/bff/")
