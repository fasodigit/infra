<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# Intégration Spring Cloud Vault — auth-ms / poulets-api / notifier-ms

## pom.xml — ajouter

```xml
<dependency>
  <groupId>org.springframework.cloud</groupId>
  <artifactId>spring-cloud-starter-vault-config</artifactId>
  <version>4.1.3</version>
</dependency>
<dependency>
  <groupId>org.springframework.cloud</groupId>
  <artifactId>spring-cloud-vault-config-databases</artifactId>
  <version>4.1.3</version>
</dependency>
```

## bootstrap.yml (ordre d'évaluation PRIORITAIRE avant application.yml)

```yaml
spring:
  application:
    name: auth-ms     # key for vault KV lookup → faso/application/auth-ms + faso/auth-ms
  config:
    import: vault:/
  cloud:
    vault:
      uri: ${VAULT_ADDR:http://vault:8200}
      authentication: APPROLE
      app-role:
        role-id: ${VAULT_ROLE_ID}       # injecté via env (pas de défaut)
        secret-id: ${VAULT_SECRET_ID}
        role: faso-auth-ms
      fail-fast: true
      kv:
        enabled: true
        backend: faso
        default-context: auth-ms
      database:
        enabled: true
        role: auth-ms-readwrite
        backend: database
        static-role: false
      # Transit : encrypt/decrypt direct via VaultTemplate bean (pas auto-injection)
      transit:
        enabled: true
        default-context: jwt-key
```

## Usage dans le code

### Lecture d'un secret KV

```java
@Value("${faso.auth-ms.jwt.encryption-key-b64:}")
private String jwtEncryptionKeyB64;  // injecté depuis faso/auth-ms/jwt#encryption_key_b64
```

### DB credentials dynamiques

```yaml
spring:
  datasource:
    url: jdbc:postgresql://postgres:5432/auth_ms
    # username + password injectés automatiquement par spring-cloud-vault-config-databases
    # renouvellement automatique ~ TTL/2
```

### Transit (encrypt/decrypt PII)

```java
@Autowired
private VaultOperations vault;

public byte[] encryptPii(byte[] plaintext) {
    return vault.opsForTransit().encrypt("pii-key", Plaintext.of(plaintext)).getCiphertext().getBytes();
}

public byte[] decryptPii(String ciphertext) {
    return vault.opsForTransit().decrypt("pii-key", Ciphertext.of(ciphertext)).getPlaintext().getValue();
}
```

## Injection AppRole credentials

```bash
# Generate AppRole credentials at deployment time (wrap-time 1h).
ROLE_ID=$(vault read -field=role_id auth/approle/role/faso-auth-ms/role-id)
SECRET_ID=$(vault write -f -field=secret_id auth/approle/role/faso-auth-ms/secret-id)

# Inject into docker-compose.yml environment:
#   VAULT_ROLE_ID: ${ROLE_ID}
#   VAULT_SECRET_ID: ${SECRET_ID}
```

## Health check Vault

`/actuator/health` expose automatiquement l'état Vault via Spring Boot Actuator
(`management.health.vault.enabled=true` par défaut).

## Secrets rotation runtime

Spring Cloud Vault écoute les événements de renouvellement. Les `@RefreshScope`
beans (ex: `DataSource`) sont recréés automatiquement à l'expiration du lease.
