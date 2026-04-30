# SPDX-License-Identifier: AGPL-3.0-or-later
package faso.authz_test

import data.faso.authz

# Helper — build an OPA input object with pre-parsed JWT claims.
# In production ARMAGEDDON sets `input.jwt_payload` after verifying the
# signature, so policy tests don't need to forge real signed tokens.
mock_input(path, method, claims) := {
    "path": path,
    "method": method,
    "headers": {"authorization": "Bearer test-token"},
    "jwt_payload": claims,
}

# ── Public routes ─────────────────────────────────────────────────────
test_public_landing_allowed if {
    authz.allow with input as {"path": "/", "method": "GET", "headers": {}}
}

test_public_login_allowed if {
    authz.allow with input as {"path": "/auth/login", "method": "POST", "headers": {}}
}

# ── Anonymous denied on protected ─────────────────────────────────────
test_anonymous_denied_on_api if {
    not authz.allow with input as {"path": "/api/annonces", "method": "POST", "headers": {}}
}

test_anonymous_denied_on_admin if {
    not authz.allow with input as {"path": "/admin/users", "method": "GET", "headers": {}}
}

# ── Eleveur can POST annonces ─────────────────────────────────────────
test_eleveur_can_post_annonce if {
    authz.allow with input as mock_input(
        "/api/annonces", "POST", {"sub": "u1", "roles": ["eleveur"]}
    )
}

# ── Client cannot POST annonces ───────────────────────────────────────
test_client_cannot_post_annonce if {
    not authz.allow with input as mock_input(
        "/api/annonces", "POST", {"sub": "u2", "roles": ["client"]}
    )
}

# ── Client can POST demandes ──────────────────────────────────────────
test_client_can_post_demande if {
    authz.allow with input as mock_input(
        "/api/besoins", "POST", {"sub": "u3", "roles": ["client"]}
    )
}

# ── Admin can list users ──────────────────────────────────────────────
test_admin_can_list_users if {
    authz.allow with input as mock_input(
        "/admin/users", "GET", {"sub": "admin1", "roles": ["ADMIN"]}
    )
}

test_eleveur_cannot_admin if {
    not authz.allow with input as mock_input(
        "/admin/users", "GET", {"sub": "u1", "roles": ["eleveur"]}
    )
}

# ── Vétérinaire can certify halal ─────────────────────────────────────
test_vet_can_halal if {
    authz.allow with input as mock_input(
        "/api/halal/certify", "POST", {"sub": "v1", "roles": ["veterinaire"]}
    )
}

# ── Marketplace browse — any authenticated ────────────────────────────
test_any_authenticated_can_browse if {
    authz.allow with input as mock_input(
        "/api/annonces?page=1", "GET", {"sub": "u4", "roles": ["client"]}
    )
}
