# SPDX-License-Identifier: AGPL-3.0-or-later
package faso.keto_data

import future.keywords.if
import future.keywords.in

# ──────────────────────────────────────────────────────────────────────
# Bridge to Ory Keto (Zanzibar relation tuples).
# OPA queries Keto's read API to materialise relations during decisions.
# Result is cached 30s by `http.send` to avoid hot-path round-trips.
# ──────────────────────────────────────────────────────────────────────

keto_read_endpoint := "http://faso-keto:4466"

# True if the user has the given relation on (namespace, object).
has_relation(user_id, namespace, relation, object) if {
    resp := http.send({
        "method": "GET",
        "url": sprintf("%s/relation-tuples/check?namespace=%s&relation=%s&object=%s&subject_id=%s",
            [keto_read_endpoint, namespace, relation, object, user_id]),
        "cache": true,
        "force_cache_duration_seconds": 30,
    })
    resp.status_code == 200
    resp.body.allowed == true
}

# Convenience predicates for the FASO domain
can_publish_offer(user_id) if has_relation(user_id, "marketplace", "publish", "offers")
can_post_demand(user_id)   if has_relation(user_id, "marketplace", "publish", "demands")
can_create_order(user_id)  if has_relation(user_id, "commerce", "purchase", "orders")
can_admin_users(user_id)   if has_relation(user_id, "admin", "manage", "users")
