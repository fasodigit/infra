# SPDX-License-Identifier: AGPL-3.0-or-later
# Vault Agent — auth-ms sidecar.
# Renders dynamic creds + JWT encryption key into a Spring-readable file.

pid_file = "/var/run/vault-agent/pidfile"

vault {
  address = "http://127.0.0.1:8200"
  retry {
    num_retries = 5
  }
}

auto_auth {
  method "approle" {
    config = {
      role_id_file_path                   = "/etc/vault-agent/auth-ms-role-id"
      secret_id_file_path                 = "/etc/vault-agent/auth-ms-secret-id"
      remove_secret_id_file_after_reading = false
    }
  }
  sink "file" {
    config = { path = "/var/run/vault-agent/auth-ms-token" }
  }
}

# Render runtime DB creds (DML, 1h TTL, auto-renew at T-15min)
template {
  source       = "/etc/vault-agent/templates/db-runtime.tpl"
  destination  = "/var/run/vault-agent/auth-ms/datasource.properties"
  error_on_missing_key = true
  perms        = "0640"
}

# Render Flyway DDL creds (one-shot at boot, 30min TTL)
template {
  source       = "/etc/vault-agent/templates/db-flyway.tpl"
  destination  = "/var/run/vault-agent/auth-ms/flyway.properties"
  error_on_missing_key = true
  perms        = "0640"
}

# Render JWT encryption key from KV v2 (auth-ms specific)
template {
  source       = "/etc/vault-agent/templates/jwt-encryption-key.tpl"
  destination  = "/var/run/vault-agent/auth-ms/jwt.properties"
  error_on_missing_key = true
  perms        = "0640"
}

# Spring Cloud Refresh: when DB creds rotate, signal the JVM to refresh
# the HikariCP pool. SIGHUP triggers RefreshScopeRefreshedEvent in our
# auth-ms thanks to ContextRefresher in Spring Cloud Vault.
exec {
  command = ["/bin/sh", "-c", "[ -f /var/run/auth-ms/pid ] && kill -HUP $(cat /var/run/auth-ms/pid) || true"]
}
