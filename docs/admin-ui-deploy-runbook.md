<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Admin-UI Phase 4.b — Runbook de déploiement

Ce runbook décrit l'ordre canonique de déploiement du sous-système admin-UI
FASO (auth-ms admin module + frontend admin + Redpanda flux audit).
Cible : poste développeur ou environnement staging avec `podman-compose`.

## Pré-requis

- Stack de base démarrée :
  ```bash
  cd INFRA/docker/compose
  podman-compose -f podman-compose.yml \
                 -f ../../vault/podman-compose.vault.yml up -d
  ```
- Vault initialisé (`bash INFRA/vault/scripts/init.sh`) et `VAULT_TOKEN` exporté.
- Redpanda et Schema Registry prêts (port 18081 exposé pour Schema Registry).
- Postgres opérationnel pour `auth-ms` (port 5432 ou 5433 selon override).

## Ordre de déploiement (5 étapes)

### 1. Init secrets Vault

```bash
bash INFRA/vault/scripts/seed-admin-secrets.sh
```

Pousse sous `faso/auth-ms/` :

| Path                              | Usage                                         |
| --------------------------------- | --------------------------------------------- |
| `otp-hmac-key`                    | HMAC-SHA-256 pour digestion OTP en DB         |
| `totp-master-secret`              | AES-GCM master pour chiffrer les seeds TOTP   |
| `recovery-codes-pepper`           | Pepper pour bcrypt des recovery codes         |
| `break-glass-master-key`          | Clé maître JWT break-glass (HS512)            |
| `webauthn-rp-id`                  | RP ID WebAuthn (`faso.bf`)                    |
| `redpanda-bootstrap`              | `redpanda:9092`                               |
| `kratos-internal-token`           | Bearer Kratos → auth-ms (webhooks login/reg)  |

Le token Kratos doit ensuite être propagé en variable d'env du conteneur
Kratos (`KRATOS_INTERNAL_TOKEN`) pour que les webhooks soient authentifiés.

### 2. Création des topics Redpanda

```bash
bash INFRA/scripts/seed-redpanda-admin-topics.sh
```

Le script utilise `rpk` local s'il est dans le `PATH`, sinon retombe sur
`podman exec redpanda rpk`. Il crée 9 topics fonctionnels + 4 DLQ avec
des rétentions adaptées au caractère légal/audit des événements
(7 jours pour les flux opérationnels, 90 jours pour les changements de
rôles, 1 an pour le break-glass, ~7 ans pour les settings).

Les schémas Avro associés se trouvent dans `INFRA/redpanda/schemas/` et
sont enregistrés à la main (ou via CI) sur le Schema Registry :18081.

### 3. Migrations DB auth-ms

```bash
cd INFRA/auth-ms
mvn flyway:migrate
```

Crée les tables admin (`admin_otp`, `admin_recovery_code`, `admin_device_trust`,
`admin_audit_log`, `admin_break_glass_session`).

### 4. Tuples Keto (super-admins initiaux)

```bash
export SEED_SA_AMINATA="<uuid-Kratos-Aminata>"
export SEED_SA_SOULEYMANE="<uuid-Kratos-Souleymane>"
bash INFRA/ory/keto/scripts/seed-admin-tuples.sh
```

Pré-requis : les deux identités doivent déjà exister dans Kratos
(création via `/auth/registration` ou via Kratos Admin API :4434).
Le script écrit deux tuples `AdminRole:platform#super_admin@<uuid>` via
le Keto Write API :4467.

### 5. Restart des services impactés

```bash
cd INFRA/docker/compose
podman-compose -f podman-compose.yml restart auth-ms notifier-ms armageddon
```

Le redémarrage est nécessaire pour que :
- `auth-ms` recharge les secrets Vault (mode lazy fetch + cache),
- `notifier-ms` recharge la config SMTP / canaux OTP,
- `armageddon` recharge ses routes Keto pour le namespace `AdminRole`.

## Validations post-déploiement

```bash
# Vault
vault kv list faso/auth-ms/                                  # 7 entries

# Redpanda
podman exec redpanda rpk topic list | grep -E '^(auth|admin)\.'

# Keto — vérifier les tuples super_admin
curl -s 'http://127.0.0.1:4466/relation-tuples?namespace=AdminRole&object=platform' | jq

# auth-ms healthcheck
curl -fsS http://127.0.0.1:8801/actuator/health | jq .status

# Kratos webauthn endpoint actif ?
curl -fsS http://127.0.0.1:4433/health/ready
```

## Notes Phase 4.b

- **OTP 8 chiffres** : Kratos `code` flow ne supporte que 6 chiffres en
  upstream. La décision de Stream D2 est de **servir l'OTP admin
  8-chiffres via auth-ms (`OtpService`)** plutôt que de patcher Kratos.
  Le `code` flow Kratos reste pour les flux non-admin (recovery /
  verification standards).
- **WebAuthn** : la méthode est activée pour l'ensemble du selfservice
  Kratos. Côté frontend admin, on appellera `selfservice/settings`
  pour enrôler la clé hardware avant tout accès admin sensible (policy
  enforcement côté ARMAGEDDON via Keto `manage_users`).
- **Webhooks Kratos** : le bearer `KRATOS_INTERNAL_TOKEN` est le secret
  partagé Kratos/auth-ms. Rotation : `vault kv put faso/auth-ms/kratos-internal-token`
  puis restart Kratos + auth-ms.

## Rollback

1. Désactiver les webhooks dans `kratos.yml` (commenter `web_hook` blocs).
2. Supprimer les tuples : `curl -X DELETE` sur Keto `/admin/relation-tuples`.
3. Drop tables admin : `mvn flyway:undo` (requiert Flyway Teams) ou rollback
   manuel via les SQL `*-undo.sql`.
4. Restart Kratos + auth-ms.
