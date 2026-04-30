# SPDX-License-Identifier: AGPL-3.0-or-later
# Vault Agent — poulets-api sidecar.

pid_file = "/var/run/vault-agent/pidfile-poulets"

vault {
  address = "http://127.0.0.1:8200"
}

auto_auth {
  method "approle" {
    config = {
      role_id_file_path   = "/etc/vault-agent/poulets-api-role-id"
      secret_id_file_path = "/etc/vault-agent/poulets-api-secret-id"
    }
  }
  sink "file" { config = { path = "/var/run/vault-agent/poulets-api-token" } }
}

template {
  source              = "/etc/vault-agent/templates/db-runtime.tpl"
  destination         = "/var/run/vault-agent/poulets-api/datasource.properties"
  error_on_missing_key = true
  perms               = "0640"
}

template {
  source              = "/etc/vault-agent/templates/db-flyway.tpl"
  destination         = "/var/run/vault-agent/poulets-api/flyway.properties"
  error_on_missing_key = true
  perms               = "0640"
}

exec {
  command = ["/bin/sh", "-c", "[ -f /var/run/poulets-api/pid ] && kill -HUP $(cat /var/run/poulets-api/pid) || true"]
}
