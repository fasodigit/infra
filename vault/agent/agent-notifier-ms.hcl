# SPDX-License-Identifier: AGPL-3.0-or-later
# Vault Agent — notifier-ms sidecar.

pid_file = "/var/run/vault-agent/pidfile-notifier"

vault {
  address = "http://127.0.0.1:8200"
}

auto_auth {
  method "approle" {
    config = {
      role_id_file_path   = "/etc/vault-agent/notifier-ms-role-id"
      secret_id_file_path = "/etc/vault-agent/notifier-ms-secret-id"
    }
  }
  sink "file" { config = { path = "/var/run/vault-agent/notifier-ms-token" } }
}

template {
  source              = "/etc/vault-agent/templates/db-runtime.tpl"
  destination         = "/var/run/vault-agent/notifier-ms/datasource.properties"
  error_on_missing_key = true
  perms               = "0640"
}

template {
  source              = "/etc/vault-agent/templates/db-flyway.tpl"
  destination         = "/var/run/vault-agent/notifier-ms/flyway.properties"
  error_on_missing_key = true
  perms               = "0640"
}

# SMTP credentials for Mailpit/Mailersend
template {
  contents = <<-EOT
    {{ with secret "faso/data/notifier-ms/smtp" }}
    spring.mail.username={{ .Data.data.username }}
    spring.mail.password={{ .Data.data.password }}
    {{ end }}
  EOT
  destination = "/var/run/vault-agent/notifier-ms/smtp.properties"
  perms       = "0640"
}

exec {
  command = ["/bin/sh", "-c", "[ -f /var/run/notifier-ms/pid ] && kill -HUP $(cat /var/run/notifier-ms/pid) || true"]
}
