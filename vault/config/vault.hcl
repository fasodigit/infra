# SPDX-License-Identifier: AGPL-3.0-or-later
# Vault main server configuration — Consul backend for FASO DIGITALISATION.

ui            = true
cluster_name  = "faso-vault-ouaga-1"
disable_mlock = false

# HTTP listener (dev / local). For production, enable TLS block.
listener "tcp" {
  address       = "0.0.0.0:8200"
  tls_disable   = true
  # tls_cert_file = "/vault/tls/vault.crt"
  # tls_key_file  = "/vault/tls/vault.key"
  # tls_min_version = "tls13"
}

# Storage backend: Consul (replicable, native Vault HA).
storage "consul" {
  address = "consul:8500"
  path    = "vault/"
  scheme  = "http"
  # token = "<consul-acl-token>"   # for production with ACLs enabled
}

# Clustering (HA). For production, replicate Vault across 3 nodes.
api_addr     = "http://vault:8200"
cluster_addr = "http://vault:8201"

# Telemetry — Prometheus-compatible.
telemetry {
  prometheus_retention_time = "30s"
  disable_hostname          = true
}

# Logging
log_level = "Info"
log_file  = "/vault/logs/vault.log"

# Explicit max lease TTLs (override per-mount as needed).
default_lease_ttl = "168h"   # 7 days
max_lease_ttl     = "720h"   # 30 days
