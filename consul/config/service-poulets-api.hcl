# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# ============================================================
# Consul service registration — poulets-api (Chicken platform API)
# Port: 8901 (HTTP) | Health: 9001/actuator/health (actuator)
# ============================================================

service {
  name = "poulets-api"
  port = 8901
  tags = ["java", "grpc", "production"]

  meta {
    version     = "1.0.0"
    team        = "backend-team"
    datacenter  = "bf-ouaga-1"
  }

  check {
    http     = "http://localhost:9001/actuator/health"
    interval = "10s"
    timeout  = "3s"
    deregister_critical_service_after = "90s"
  }

  connect {
    sidecar_service {}
  }
}
