<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Phase 4.a — Gap analysis admin-UI v2 (consolidé depuis 2 explorations parallèles) -->

# Gap Analysis — Admin-UI v2 · Phase 4.a

**Date** : 2026-04-30
**Source 1** : extraction contrat frontend (52 endpoints sur 12 domaines)
**Source 2** : inventaire backend (auth-ms, BFF, notifier-ms, Kratos, Keto, KAYA, Redpanda, Vault, ARMAGEDDON, E2E)
**Couverture globale actuelle** : **~35 %**

---

## TL;DR

La couche identité existe (Kratos + Keto + KAYA opérationnels), JWT ES384 + bruteforce + JTI blacklist en place. **MAIS** : pas de producer Redpanda dans auth-ms, aucun controller admin, 7 migrations Flyway à écrire, 8 services métier à créer, namespace Keto `AdminRole` absent, dépendances WebAuthn + spring-kafka manquantes au pom.xml, BFF n'a que `/api/admin/workflows/*`, notifier-ms n'a aucun template OTP/admin.

**Verdict** : architecture bien posée, **0 % du périmètre admin-UI v2 implémenté**. Effort estimé : 5-6 semaines en parallèle (cf. §13).

---

## 1. Couverture endpoints (52 endpoints frontend → backend)

| Domaine | Endpoints | ✅ existants | ❌ à créer |
|---|---:|---:|---:|
| Users | 8 | 0 | 8 |
| Sessions | 3 | 0 | 3 |
| Devices | 3 | 0 | 3 |
| MFA · PassKey | 4 | 0 | 4 |
| MFA · TOTP | 3 | 0 | 3 |
| Recovery codes | 2 | 0 | 2 |
| Audit | 3 | 0 | 3 |
| Settings (Configuration Center) | 5 | 0 | 5 |
| OTP | 2 | 0 | 2 |
| Roles & grants | 3 | 0 | 3 |
| Break-Glass | 3 | 0 | 3 |
| Dashboard | 1 | 0 | 1 |
| Bonus (couverture future) | 6 | 0 | 6 |
| **TOTAL** | **52 + 6** | **0** | **52 + 6** |

**Verdict** : aucun endpoint admin-UI v2 n'est servi à ce jour. Tous à créer côté BFF Next.js + auth-ms.

---

## 2. Migrations Flyway à créer (auth-ms)

Existant : `V1__init.sql` (users, roles, permissions, audit_log, jwt_signing_keys), `V2__encrypt_jwt_keys.sql`.

| Version | Fichier | Tables / Alter |
|---|---|---|
| V3 | `V3__totp_enrollments.sql` | `totp_enrollments(id, user_id, secret_encrypted AES-256-GCM, enrolled_at, disabled_at)` |
| V4 | `V4__recovery_codes.sql` | `recovery_codes(id, user_id, code_hash bcrypt, used_at, generated_at, expires_at)` + index partiel `WHERE used_at IS NULL` |
| V5 | `V5__device_registrations.sql` | `device_registrations(id, user_id, fingerprint, device_type, public_key_pem, ua_string, ip_address, created_at, last_used_at, trusted_at, revoked_at)` UNIQUE(user_id, fingerprint) |
| V6 | `V6__admin_role_grants.sql` | `admin_role_grants(id, grantor_id, grantee_id, role_id, justification, status PENDING/APPROVED/REJECTED/EXPIRED, approver_id, expires_at, created_at, approved_at)` + index status PENDING |
| V7 | `V7__admin_settings.sql` | `admin_settings(key, value JSONB, value_type, category, min/max/default, required_role_to_edit, version, updated_at, updated_by)` + `admin_settings_history(id, key, version, old_value, new_value, motif, changed_by, changed_at, trace_id)` UNIQUE(key, version) |
| V8 | `V8__mfa_status.sql` | `mfa_status(user_id PK, totp_enabled, passkey_count, backup_codes_remaining, trusted_devices_count, updated_at)` |
| V9 | `V9__audit_log_extend.sql` | `ALTER audit_log ADD resource_type, old_value JSONB, new_value JSONB, metadata JSONB, trace_id, user_agent` + 3 index |

**Seed** : tuples Keto SUPER-ADMIN/ADMIN/MANAGER + seed `admin_settings` (38 paramètres × 6 catégories).

---

## 3. Dépendances Maven (`auth-ms/pom.xml`)

| Dépendance | Version | Raison |
|---|---|---|
| `com.yubico:webauthn-server-core` | ≥ 0.20.0 | PassKey / WebAuthn FIDO2 |
| `dev.samstevens.totp:totp-spring-boot-starter` | 1.7.1 | TOTP RFC 6238 (alternative : `org.jboss.aerogear:aerogear-otp-java`) |
| `org.springframework.kafka:spring-kafka` | aligné Spring Boot 3.4 | Producer Redpanda |
| (optionnel) `io.micrometer:micrometer-tracing-bridge-otel` | 1.4.x | Spans OTel propagés Kafka |

**Note** : `spring-data-redis` (lettuce) déjà présent → KAYA RESP3 OK.

---

## 4. Services métier à créer (auth-ms)

| Service | Fichier | Responsabilité |
|---|---|---|
| `OtpService` | `service/OtpService.java` | Génération 8 chiffres `SecureRandom`, hash HMAC-SHA256, stockage KAYA `auth:otp:{otpId}` TTL 300s, rate-limit `auth:otp:rl:{userId}`, lock `auth:otp:lock:{userId}` |
| `TotpService` | `service/TotpService.java` | Enrollment (secret base32 chiffré AES-256-GCM), QR code URL `otpauth://`, verify code 6 chiffres window=1 |
| `WebAuthnService` | `service/WebAuthnService.java` | yubico/webauthn — registration begin/finish, authentication begin/finish, challenge KAYA `auth:passkey:pending:{userId}` TTL 600s |
| `RecoveryCodeService` | `service/RecoveryCodeService.java` | Génération 10 codes single-use (format XXXX-XXXX), hash bcrypt, stockage DB |
| `DeviceTrustService` | `service/DeviceTrustService.java` | Fingerprint `SHA-256(UA + IP/24 + Accept-Language)`, KAYA `dev:{userId}:{fp}` TTL 30j (configurable via settings) |
| `AdminSettingsService` | `service/AdminSettingsService.java` | CRUD avec optimistic concurrency (CAS sur `version`), historique, publish `admin.settings.changed`, rollback |
| `BreakGlassService` | `service/BreakGlassService.java` | Élévation 4h via KAYA `auth:break_glass:{userId}` TTL 14400s + tuple Keto temporaire `super_admin@user`, scheduler auto-révocation, notification SUPER-ADMIN |
| `AdminMfaEnrollmentService` | `service/AdminMfaEnrollmentService.java` | Orchestration : OTP → PassKey OR TOTP → 10 backup codes, transition `mfa_status` |
| `AdminAuditService` | `service/AdminAuditService.java` | INSERT audit_log + publish event Redpanda async (DLQ si échec), query avec filtres |
| `AdminRoleGrantService` | (étend `PermissionGrantService`) | Workflow dual-control : create `admin_role_grants(PENDING)`, OTP, approval link SA, sync Keto sur APPROVED |

**Existant à réutiliser** : `JwtService`, `KetoService`, `BruteForceService`, `JtiBlacklistService`, `SessionLimitService`, `KratosService`, `PermissionGrantService` (à étendre).

---

## 5. Controllers REST à créer (auth-ms)

| Controller | Path racine | Endpoints (cf. §1) |
|---|---|---|
| `AdminUserController` | `/admin/users` | GET list, POST invite, GET detail, POST suspend, DELETE suspend, POST mfa/reset |
| `AdminRoleController` | `/admin/users/{id}/roles` | POST grant, POST revoke |
| `AdminSessionController` | `/admin/sessions` | GET list, DELETE one, DELETE all |
| `AdminDeviceController` | `/admin/devices` | GET list, POST :id/trust, DELETE :id |
| `AdminPasskeyController` | `/admin/users/{id}/passkeys` | POST enroll/begin, POST enroll/finish, DELETE :passkeyId, POST :passkeyId/rename |
| `AdminTotpController` | `/admin/users/{id}/totp` | POST enroll/begin, POST enroll/finish, DELETE |
| `AdminRecoveryCodeController` | `/admin/recovery-codes` | POST generate, POST use |
| `AdminOtpController` | `/admin/otp` | POST issue, POST verify |
| `AdminAuditController` | `/admin/audit` | GET query, GET :id, POST export.csv |
| `AdminSettingsController` | `/admin/settings` | GET all, GET :key, PUT :key (CAS), GET :key/history, POST :key/revert |
| `AdminBreakGlassController` | `/admin/break-glass` | POST activate, GET status, POST revoke |
| `AdminDashboardController` | `/admin/dashboard` | GET kpis |

**Sécurité** : annotation `@PreAuthorize("hasRole('SUPER-ADMIN')")` ou check Keto inline via `KetoService` selon endpoint. JWT validé par `JwtAuthenticationFilter` existant. Trace propagée via `traceparent`.

---

## 6. Topics Redpanda à créer

Existant : `github.events.v1` + DLQ. Auto-create ON. Schema Registry actif :18081.

| Topic | Partitions | Rétention | Producer | Consumer |
|---|---:|---|---|---|
| `auth.otp.issue` | 3 | 7d | auth-ms `OtpService` | notifier-ms (envoi mail) |
| `auth.otp.verified` | 3 | 30d | auth-ms | analytics |
| `auth.role.granted` | 1 | 90d | auth-ms | notifier-ms (email cible) + ARMAGEDDON (cache invalidate) |
| `auth.role.revoked` | 1 | 90d | auth-ms | ARMAGEDDON |
| `auth.device.trusted` | 3 | 30d | auth-ms | analytics |
| `auth.session.revoked` | 3 | 7d | auth-ms | ARMAGEDDON (kill JWT) |
| `admin.break_glass.activated` | 1 | 365d | auth-ms | notifier-ms (alerte SA) + audit immutable |
| `admin.settings.changed` | 1 | 2555d (7 ans Loi 010-2004 BF) | auth-ms | ARMAGEDDON + KAYA + audit immutable |
| `admin.user.suspended` / `reactivated` | 3 | 7d | auth-ms | notifier-ms |

Schémas Avro à enregistrer (Schema Registry :18081). Pattern de producer : `KafkaTemplate<String, SpecificRecord>` avec retry + idempotent producer (`enable.idempotence=true`).

---

## 7. Préfixes KAYA à ajouter

Existant : `auth:jti:blacklist:*`, `auth:sessions:*`, `auth:bruteforce:*`, `poulets:*`.

| Préfixe | Type | TTL | Service |
|---|---|---|---|
| `auth:otp:{otpId}` | HASH | 300s | OtpService |
| `auth:otp:rl:{userId}` | STRING (counter) | 300s | OtpService rate-limit |
| `auth:otp:lock:{userId}` | STRING (marker) | 900s | OtpService post-fail |
| `auth:totp:temp:{userId}` | HASH | 600s | TotpService enrollment |
| `auth:passkey:pending:{userId}` | HASH | 600s | WebAuthnService challenge |
| `auth:recovery:{userId}` | HASH | 31536000s | RecoveryCodeService meta |
| `dev:{userId}:{fp}` | HASH | 2592000s (30j) | DeviceTrustService |
| `auth:break_glass:{userId}` | HASH | 14400s (4h) | BreakGlassService |
| `admin:settings:cache:{key}` | STRING | 30s | BFF cache (côté Next.js, pas auth-ms) |

---

## 8. Keto — namespace `AdminRole` à créer

Fichier : `INFRA/ory/keto/config/namespaces.ts` (à étendre).

```typescript
class AdminRole implements Namespace {
  related: { super_admin: User[]; admin: User[]; manager: User[] }
  permits = {
    grant_admin_role: (ctx) => this.related.super_admin.includes(ctx.subject),
    grant_manager_role: (ctx) =>
      this.related.super_admin.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),
    manage_users: (ctx) =>
      this.related.super_admin.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),
    view_audit: (ctx) =>
      this.related.super_admin.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject) ||
      this.related.manager.includes(ctx.subject),
    update_settings: (ctx) => this.related.super_admin.includes(ctx.subject),
    activate_break_glass: (ctx) =>
      this.related.super_admin.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),
  }
}
```

**Seed tuples** : script `INFRA/ory/keto/scripts/seed-admin-tuples.sh` qui appelle Keto write API avec les SUPER-ADMIN initiaux (Aminata, Souleymane).

---

## 9. Kratos — extensions

État actuel : flows `password`, `totp`, `code` (15min lifespan), `link` activés. Identity schema a déjà le trait `role` (admin/operator/eleveur/client).

| Modification | Action |
|---|---|
| Identity schema `role` | Étendre l'enum : ajouter `super-admin`, `manager` (en plus de `admin`) — fichier `ory/kratos/schemas/identity.schema.json` |
| Flow `webauthn` | Activer dans `ory/kratos/config/kratos.yml` (méthode WebAuthn FIDO2) |
| Hooks post-login | Webhook → auth-ms `/internal/admin/login-event` pour audit + device-trust check |
| Hooks post-registration | Webhook → auth-ms pour création MFA enrollment (mandatory pour ADMIN+) |
| Email templates | Personnaliser `templates/email/*` avec branding FASO + 8 chiffres OTP (au lieu de 6) |

---

## 10. notifier-ms — templates & consumers

Existant : 8 templates `*-commit.hbs` (poulets, etat-civil, hospital, sogesy, escool, eticket, altmission, fasokalan), 2 templates PR. Consumer `GithubEventConsumer` (kafka topic `github.events.v1`, DLQ, Manual ack).

### 10.1 Templates Handlebars à créer
- `otp-email.hbs` — code 8 chiffres avec branding BF
- `admin-invitation.hbs` — invitation admin avec lien token
- `admin-role-granted.hbs` — notification cible que rôle octroyé
- `admin-role-grant-approval-required.hbs` — demande approbation SA
- `admin-mfa-enrollment-instruction.hbs`
- `admin-recovery-codes.hbs` — fichier .txt en pièce jointe
- `admin-break-glass-activated.hbs` — alerte SA (priorité haute)
- `admin-session-revoked.hbs`
- `admin-settings-changed.hbs` (digest hebdo SA)

### 10.2 Consumers à créer
- `OtpEventConsumer` — topic `auth.otp.issue` → SMTP Mailpit (dev) / SMTP prod via secret Vault
- `RoleGrantedEventConsumer` — topic `auth.role.granted` → mail cible
- `BreakGlassEventConsumer` — topic `admin.break_glass.activated` → notification all SA + Slack webhook (futur)
- `SessionRevokedEventConsumer` — topic `auth.session.revoked` → mail user (optionnel selon settings)

Pattern : réutiliser le pattern `GithubEventConsumer` (KAYA dedup, DLQ, retry context engine).

---

## 11. BFF Next.js — routes à créer

Existant : `bff/src/app/api/admin/workflows/*` (3 routes Temporal).

À créer (cf. §11 du brief Claude Design) — total 30+ route handlers :
- `dashboard/route.ts`
- `users/route.ts`, `users/[userId]/route.ts`, `users/invite/route.ts`, `users/[userId]/{suspend,mfa/reset}/route.ts`, `users/[userId]/roles/{grant,revoke}/route.ts`
- `sessions/route.ts`, `sessions/[sessionId]/route.ts`
- `devices/route.ts`, `devices/[deviceId]/{trust,/}/route.ts`
- `users/[userId]/passkeys/{enroll/begin,enroll/finish,[id],[id]/rename}/route.ts`
- `users/[userId]/totp/{enroll/begin,enroll/finish,/}/route.ts`
- `recovery-codes/{generate,use}/route.ts`
- `audit/route.ts`, `audit/[id]/route.ts`, `audit/export.csv/route.ts`
- `settings/route.ts`, `settings/[key]/route.ts`, `settings/[key]/history/route.ts`, `settings/[key]/revert/route.ts`
- `otp/{issue,verify}/route.ts`
- `break-glass/{activate,status,revoke}/route.ts`

**Middleware** `lib/admin-auth.ts` : verify Kratos session + JWT + Keto check.
**Lib** `lib/admin-audit.ts`, `lib/admin-otp.ts`, `lib/schemas/admin.ts` (Zod).

---

## 12. ARMAGEDDON gateway

Aucune route `/admin/*` déclarée. À ajouter dans config xDS / manifest YAML :

```yaml
routes:
  - match: { prefix: "/api/admin/" }
    route:
      cluster: bff_admin                  # → poulets-platform/bff:4800
      timeout: 30s
    typed_per_filter_config:
      ext_authz:
        check_settings:
          context_extensions: { admin_route: "true" }
  # ext_authz vers Keto :4466 sur tous les /admin/*
```

---

## 13. Vault — secrets à seeder

Existant : `faso/auth-ms/JWT_KEY_ENCRYPTION_KEY`, `KAYA_PASSWORD`.

| Path | Type | Usage |
|---|---|---|
| `faso/auth-ms/otp-hmac-key` | 32 bytes random | HMAC-SHA256 sur OTP avant stockage |
| `faso/auth-ms/totp-master-secret` | 32 bytes | Master key AES-256 chiffrement secrets TOTP en DB |
| `faso/auth-ms/recovery-codes-pepper` | 32 bytes | Pepper bcrypt sur recovery codes |
| `faso/auth-ms/break-glass-master-key` | 32 bytes | Chiffrement justifications break-glass en audit |
| `faso/auth-ms/webauthn-rp-id` | string | "faso.bf" (relying party ID) |
| `faso/auth-ms/redpanda-bootstrap` | string | `redpanda:9092` (alias) |

Script seed à créer : `INFRA/vault/scripts/seed-admin-secrets.sh`.

---

## 14. Tests E2E (cf. Phase 4.c — préparation)

Spécifications déjà cataloguées (cf. brief §13) : 13+ specs sous `tests/18-admin-workflows/` + 2 specs settings (#14, #15).

Fixtures existantes à enrichir :
- `actors.ts` : ajouter rôle `SUPER-ADMIN`
- `mailpit.ts` : `waitForOtp(email, { regex: /\b(\d{8})\b/ })`
- `session.ts` : `loginWithOtp()`, `loginWithPasskey()`, `loginWithTotp()`, `loginWithRecoveryCode()`
- Nouveau `device-trust.ts`
- Nouveau page object `AdminDashboardPage.ts`

---

## 15. Plan d'implémentation Phase 4.b (5-6 semaines, agents parallèles)

| Semaine | Stream A — Identity | Stream B — Bus & Cache | Stream C — UI BFF | Stream D — Infra |
|---|---|---|---|---|
| 1 | Migrations V3-V5 + entities `TotpEnrollment`, `RecoveryCode`, `DeviceRegistration`. `OtpService` + `TotpService`. | Topics Redpanda (8). Producer auth-ms `KafkaTemplate`. Schémas Avro. | Routes BFF `/api/admin/{otp,users}` (skeletons). Middleware auth+Keto. | pom.xml ajouts (yubico, spring-kafka, totp). Vault seeds. |
| 2 | `WebAuthnService` + `RecoveryCodeService`. Controllers `AdminPasskey/Totp/Recovery`. | Consumer notifier-ms `OtpEventConsumer`. Templates `otp-email.hbs` + `admin-invitation.hbs`. | Routes BFF MFA + recovery. Composants frontend câblés (remplacer mocks). | Keto namespace `AdminRole` + seed tuples SA. |
| 3 | Migrations V6-V8 + entities `AdminRoleGrant`, `MfaStatus`. `AdminRoleGrantService`. Controller `AdminRole`. | Consumer `RoleGrantedEventConsumer`. Templates role-granted. | Routes BFF `/api/admin/{users/[id]/roles, audit, sessions, devices}`. | Kratos webhook post-login → audit. |
| 4 | Migration V7 (settings + history) + `AdminSettingsService` (CAS, history, revert). Controller `AdminSettings`. `BreakGlassService`. | Consumer `BreakGlassEventConsumer` + alerte SA. Schedule auto-révocation 4h. | Routes BFF settings (PUT CAS, history, revert) + break-glass. | ARMAGEDDON routes /admin/* + ext_authz Keto. |
| 5 | `AdminAuditService` extend (filtres, export CSV/JSON). `AdminDashboardController`. | Topic `admin.settings.changed` consumer ARMAGEDDON (cache invalidate). | Tous les endpoints câblés. Cache 30s côté BFF. | Migration V9 audit_log columns. Seed `admin_settings` 38 paramètres. |
| 6 | Bugfix + hardening sécurité (request signing HMAC, audit immutable). | Load tests P99 (10k OTP/min). | Polish UI + i18n. | Phase 4.c E2E + 4.d cycle-fix. |

---

## 16. Risques & mitigations

| Risque | Sévérité | Mitigation |
|---|---|---|
| Auto-create topics Redpanda en prod | HAUTE | Désactiver `auto.create.topics.enable=false` en prod, seeds explicites via `rpk` |
| Drift settings cache vs DB | MOYENNE | Topic `admin.settings.changed` + invalidation cache BFF/ARMAGEDDON |
| Race condition sur grant dual-control | HAUTE | Transaction DB + state machine (PENDING → APPROVED) |
| Replay attack sur OTP | CRITIQUE | OTP single-use + KAYA delete after verify |
| Brute-force device fingerprint | MOYENNE | Rate-limit + fingerprint hashed (32 hex) |
| Break-glass auto-révocation manquée | HAUTE | Quartz scheduler + scan KAYA TTL + alarme Prom si dépassement |
| Flyway versionning legacy | BASSE | V1, V2 actuels sont conformes — V3+ réservés admin |

---

## 17. Décisions de design à valider

1. **WebAuthn lib** : `com.yubico:webauthn-server-core` (proposé) vs `com.webauthn4j:webauthn4j-core`. *Recommandation : yubico, plus mature.*
2. **TOTP lib** : `dev.samstevens.totp:totp-spring-boot-starter` (proposé) vs `org.jboss.aerogear:aerogear-otp-java`. *Recommandation : samstevens, intégration Spring Boot starter.*
3. **Schema Registry** : Avro vs Protobuf vs JSON Schema pour topics admin. *Recommandation : **Avro** (cohérence avec FASO existant + schema evolution).*
4. **Audit immutable** : tablespace WORM PostgreSQL OU table append-only avec trigger RAISE EXCEPTION sur UPDATE/DELETE. *Recommandation : **trigger** (portable, pas de tablespace dédié à gérer).*
5. **Quartz vs ShedLock** pour break-glass auto-révocation : *Recommandation : **ShedLock** (plus léger, KAYA-backed).*
6. **Cache BFF settings** : in-memory Node OU KAYA. *Recommandation : **KAYA** (cohérence multi-instance BFF).*

---

*Document consolidé Phase 4.a — gap analysis admin-UI v2. Couverture actuelle 35 %. 5-6 semaines pour atteindre 100 %.*
