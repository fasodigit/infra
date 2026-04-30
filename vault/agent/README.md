# Vault Agent — sidecar config FASO

## Architecture

Chaque service Java a un Vault Agent dédié qui :

1. S'authentifie via AppRole (role-id + secret-id sur disque, lecture seule)
2. Renouvelle automatiquement les credentials dynamiques DB (TTL 1h, renew T-15min)
3. Rend les properties dans `/var/run/vault-agent/<svc>/*.properties`
4. Signal `SIGHUP` au JVM Spring lors du renouvellement → refresh HikariCP

## Démarrage local (dev)

```bash
# 1. Init Vault + AppRole + DB engine + KV
cd INFRA/vault
bash scripts/init.sh
export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
bash scripts/setup-database-engine.sh        # crée runtime+flyway roles
bash scripts/setup-approle.sh                 # crée approles + policies
bash scripts/seed-secrets.sh                  # KV: jwt key, smtp creds

# 2. Récupérer le role-id et secret-id pour chaque service
for svc in auth-ms poulets-api notifier-ms; do
  vault read -field=role_id auth/approle/role/${svc}/role-id > /etc/vault-agent/${svc}-role-id
  vault write -field=secret_id -force auth/approle/role/${svc}/secret-id > /etc/vault-agent/${svc}-secret-id
done

# 3. Lancer les agents en parallèle
mkdir -p /var/run/vault-agent/{auth-ms,poulets-api,notifier-ms}
for svc in auth-ms poulets-api notifier-ms; do
  vault agent -config=agent-${svc}.hcl > /tmp/vault-agent-${svc}.log 2>&1 &
done

# 4. Vérifier les properties rendues
ls /var/run/vault-agent/auth-ms/
# datasource.properties  flyway.properties  jwt.properties

# 5. Java services chargent ces properties via spring.config.import
#    Ex auth-ms application.yml :
#    spring:
#      config:
#        import:
#          - "optional:file:/var/run/vault-agent/auth-ms/datasource.properties"
#          - "optional:file:/var/run/vault-agent/auth-ms/flyway.properties"
#          - "optional:file:/var/run/vault-agent/auth-ms/jwt.properties"
```

## Validation

- `vault read database/creds/auth-ms-runtime-role` : creds éphémères OK
- `cat /var/run/vault-agent/auth-ms/datasource.properties` : username/password rendus
- Après 45 min : Vault Agent doit auto-renew avant expiry
- `vault_agent_template_renewals_total` (Prometheus) > 0
