# SPDX-License-Identifier: AGPL-3.0-or-later
# Consul server configuration for FASO DIGITALISATION — storage backend for Vault.

datacenter = "faso-ouaga-1"
data_dir   = "/consul/data"
log_level  = "INFO"

# Single-node bootstrap for dev; production should use 3-node quorum.
server           = true
bootstrap_expect = 1

# UI
ui_config {
  enabled = true
}

# Gossip encryption (required in production — uncomment and set via env):
# encrypt = "<consul-keygen-output>"

# ACLs — enable in production:
# acl {
#   enabled        = true
#   default_policy = "deny"
#   enable_token_persistence = true
#   tokens {
#     master = "<master-token>"
#   }
# }

# Performance tuning.
performance {
  raft_multiplier = 1
}

# Log audit trail (production only, requires enterprise):
# audit {
#   enabled = true
#   sink "file" {
#     type   = "file"
#     format = "json"
#     path   = "/consul/audit/audit.log"
#   }
# }
