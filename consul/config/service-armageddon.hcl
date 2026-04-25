# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# ============================================================
# Consul service registration — ARMAGEDDON (sovereign proxy gateway)
# Port: 8080 (HTTP/HTTPS) | Admin: 9902 (loopback)
# ============================================================

service {
  name = "armageddon"
  port = 8080
  tags = ["rust", "proxy", "production"]

  meta {
    version     = "0.1.0"
    team        = "platform-team"
    datacenter  = "bf-ouaga-1"
    protocol    = "http"
  }

  check {
    http     = "http://localhost:9902/health"
    interval = "10s"
    timeout  = "3s"
    deregister_critical_service_after = "90s"
  }

  connect {
    sidecar_service {}
  }
}
