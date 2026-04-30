# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# ============================================================
# Consul service registration — auth-ms (Authentication microservice)
# Port: 8801 (HTTP) | Health: 9002/actuator/health (actuator)
# ============================================================

service {
  name = "auth-ms"
  port = 8801
  tags = ["java", "grpc", "production"]

  meta {
    version     = "1.0.0"
    team        = "backend-team"
    datacenter  = "bf-ouaga-1"
  }

  check {
    http     = "http://localhost:9002/actuator/health"
    interval = "10s"
    timeout  = "3s"
    deregister_critical_service_after = "90s"
  }

  connect {
    sidecar_service {}
  }
}
