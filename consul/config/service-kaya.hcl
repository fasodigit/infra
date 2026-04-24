# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# ============================================================
# Consul service registration — KAYA (sovereign in-memory store)
# Port: 6380 (RESP3) | gRPC: 6381
# ============================================================

service {
  name = "kaya"
  port = 6380
  tags = ["rust", "resp3", "production"]

  meta {
    version     = "0.1.0"
    team        = "platform-team"
    datacenter  = "bf-ouaga-1"
    protocol    = "resp3"
  }

  check {
    tcp      = "localhost:6380"
    interval = "10s"
    timeout  = "3s"
    deregister_critical_service_after = "90s"
  }

  connect {
    sidecar_service {}
  }
}
