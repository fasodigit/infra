# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
# ============================================================
# Consul service registration — notifier-ms (Notification microservice)
# Port: 8803 (HTTP) | Health: 8803/actuator/health (main port)
# ============================================================

service {
  name = "notifier-ms"
  port = 8803
  tags = ["java", "kafka", "production"]

  meta {
    version     = "1.0.0-SNAPSHOT"
    team        = "backend-team"
    datacenter  = "bf-ouaga-1"
  }

  check {
    http     = "http://localhost:8803/actuator/health"
    interval = "10s"
    timeout  = "3s"
    deregister_critical_service_after = "90s"
  }

  connect {
    sidecar_service {}
  }
}
