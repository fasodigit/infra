<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Brief consolidé — admin-UI FASO DIGITALISATION pour Claude Design -->
<!-- Source : 4 explorations parallèles (frontend Angular local, backend gap analysis, GitHub patterns inter-projets, Playwright E2E) -->

# Brief Claude Design — Page d'administration FASO DIGITALISATION

## 1. Contexte produit

FASO DIGITALISATION est la plateforme numérique souveraine du Burkina Faso (AGPL-3.0-or-later, Rust + Java 21 + Angular 21). Elle fédère 9+ applications sectorielles (État-civil, Hôpital, E-Ticket, SOGESY, E-School, Vouchers, ALT-MISSION, FASO-Kalan, poulets-platform) sur une infrastructure commune souveraine : KAYA (Redis-compat Rust), ARMAGEDDON (Pingora gateway Rust), ORY Kratos/Keto, Redpanda, PostgreSQL 17.

La page `/admin/` doit permettre à 3 niveaux de rôles (`SUPER-ADMIN > ADMIN > MANAGER`) de piloter sécurité, utilisateurs, sessions et audit avec traçabilité complète. Chaque action sensible exige OTP/MFA + audit + trace Jaeger.

**Périmètre MVP** : Dashboard, gestion utilisateurs, octroi/révocation rôles (workflow dual-control), MFA enrollment (PassKey + TOTP + recovery codes), device-trust registry, audit log queryable, sessions globales + force-logout, settings sécurité, pattern Break-Glass 4h.

## 2. Stack cible

### Frontend — `poulets-platform/frontend` :4801
- **Angular 21** standalone components, signals, control flow `@if/@for/@switch`, OnPush.
- **Material Design 3** (existant dans poulets-platform — `MatCard`, `MatTable`, `MatStepper`, `MatChips`, `MatDialog`, `MatSnackBar`, `MatVirtualScrollViewport`).
- **Reactive Forms** + Zod côté BFF.
- **@simplewebauthn/browser** (PassKey).
- **qrcode** lib (TOTP QR).
- **OTel SDK Web** (browser → propagation `traceparent` vers BFF).
- **ngx-translate** (FR default + EN).
- ESLint + Prettier alignés `poulets-platform/.eslintrc.json`.
- *Alternative considérée* : Tailwind+shadcn+Zustand (utilisé par `fasodigit/e-ticket`) — **rejetée** pour cohérence avec base Angular Material existante.

### BFF — `poulets-platform/bff` :4800
- **Next.js 16** App Router, RSC, Server Actions.
- **Zod** validation.
- **jose** JWT verify (JWKS auth-ms `:8801/.well-known/jwks.json`, cache 10min).
- **@opentelemetry/sdk-node** auto-instrumentation.
- Middleware `admin-auth.ts` : check session Kratos cookie `ory_kratos_session` + JWT + Keto tuple.

### Gateway — ARMAGEDDON :8080
Pingora Rust + Keto authz inline + Coraza WAF. **Toute requête browser passe par :8080** — JAMAIS d'appel direct vers :8801 / :8901.

### Identity & Permissions
- **Kratos** :4433 (signup, login, MFA, settings — déjà configuré : password+TOTP+code(15min)+link(1h), session 8h Lax).
- **Keto** :4466 read / :4467 write (loopback) — namespaces actuels : User, Role, Platform, Resource, Department.

### Backend
- **auth-ms** Spring Boot 3.4.4 / Java 21 :8801 — User/Role/Permission entities, PermissionGrantService, KetoService (circuit breaker), JwtService ES384, BruteForceService, SessionLimitService, JtiBlacklistService, audit_log table existant (V1+V2 Flyway).
- **poulets-api** :8901.
- **notifier-ms** :8803 — consumer Redpanda + SMTP Mailpit/prod, templates Handlebar, DLQ.

### Cache / Sessions / Bus
- **KAYA** :6380 RESP3, :6381 gRPC — clés OTP, device-trust TTL, JTI blacklist, rate-limit, sessions sorted sets.
- **Redpanda** :19092 ext / :9092 int, Schema Registry :18081, Console :8090.
- **Mailpit** dev :1025 SMTP / :8025 Web+REST API.
- **Vault + Consul** pour secrets (`faso/<service>/<usage>`).

### Observabilité
Jaeger :16686, Tempo :3200, Prometheus :9090, Loki :3100, Grafana :3000, OTel collector :4317/:4318, archives MinIO :9201.

### ⚠️ Souveraineté (BLOQUANT)
**JAMAIS** mentionner ni dépendre de Redis / DragonflyDB / Envoy / NGINX+ / Istio / Okta / Auth0 / AWS / GCP / Azure. **Toujours** KAYA / ARMAGEDDON / ORY / Redpanda / MinIO / Vault.

## 3. Modèle de rôles & autorisation

### Hiérarchie stricte transitive
```
SUPER-ADMIN (level=0)  ⊃  ADMIN (level=1)  ⊃  MANAGER (level=2)
```
Le rôle de niveau N peut tout ce que peut faire le rôle N+1, plus ses propres capacités. La règle d'octroi : *un rôle ne peut être attribué que par un acteur de niveau strictement inférieur* (`grantor.level < grantee.targetLevel`).

### Matrice capacités

| Capacité | SUPER-ADMIN | ADMIN | MANAGER |
|---|---|---|---|
| Voir liste users | ✓ | ✓ | ✗ |
| Octroyer ADMIN | ✓ | ✗ | ✗ |
| Octroyer MANAGER | ✓ | ✓ | ✗ |
| Révoquer rôles | ✓ | ✓ (MANAGER only) | ✗ |
| Force-logout session | ✓ | ✓ | ✗ |
| Lire audit log | ✓ | ✓ | ✓ |
| Gérer MFA users | ✓ | ✓ | ✗ |
| Update settings sécurité | ✓ | ✗ | ✗ |
| **Break-Glass 4h** | ✓ | ✓ (avec justif. signée) | ✗ |

### Keto `AdminRole` namespace (à ajouter)
```typescript
class AdminRole implements Namespace {
  related: { super_admin: User[]; admin: User[]; manager: User[] }
  permits = {
    grant_admin_role: (ctx) => this.related.super_admin.includes(ctx.subject),
    grant_manager_role: (ctx) => this.related.super_admin.includes(ctx.subject)
                              || this.related.admin.includes(ctx.subject),
    manage_users: (ctx) => this.related.super_admin.includes(ctx.subject)
                        || this.related.admin.includes(ctx.subject),
    view_audit: (ctx) => this.related.super_admin.includes(ctx.subject)
                      || this.related.admin.includes(ctx.subject)
                      || this.related.manager.includes(ctx.subject),
  }
}
```

### Pattern matrice ALT-MISSION (à adopter)
Permission row : `(tenant_id, role, action, scope)` avec `scope ∈ { TOUS, DIRECTION }`. Toute action admin liée à un user doit vérifier que `target.tenant_id == grantor.tenant_id` sauf SUPER-ADMIN scope=TOUS.

### Pattern Break-Glass (E-SCHOOL)
Endpoint `/admin/break-glass/activate` qui élève temporairement un ADMIN en SUPER-ADMIN pendant 4h, avec :
- Justification texte obligatoire (≥ 80 caractères).
- OTP confirmation obligatoire.
- Event Redpanda `admin.break_glass.activated` → notification immédiate à tous les SUPER-ADMIN.
- Audit en mode immutable append-only (PostgreSQL WAL).
- Auto-révocation au TTL.

## 4. Pages & composants à designer

### 4.1 Dashboard — `/admin`
**KPI cards (4)** : Users actifs 7j • OTP envoyés 24h • Sessions actives globales • Alertes sécurité non-acquittées.
**Graph** : Line chart 7j (Recharts compat ou ngx-charts).
**Health grid** : ARMAGEDDON / auth-ms / KAYA / Kratos / Keto / PostgreSQL / Redpanda — couleurs (`healthy=#1b5e20`, `degraded=#FF9800`, `down=#E53935`).
**Recent timeline** : 10 dernières actions admin, click → modal détail (trace ID Jaeger).

### 4.2 Gestion utilisateurs — `/admin/users`
- Header : titre + search + filtre rôle (chips multi-select).
- Table virtualisée `cdk-virtual-scroll` (50 rows/viewport, lazy-load) — colonnes : Email, Nom, Rôle (chip coloré par niveau), Vérifié (icône), MFA (icône+count), Created, Last active, Actions.
- Inline actions : `View`, `Manage roles`, `Force logout`, `Reset MFA`, `Suspend`.
- Pagination 100/200.

### 4.3 Détail utilisateur — `/admin/users/:userId`
Sections empilées : Profil readonly • Rôles actuels (chips × bouton remove conditionnel) • Sessions actives (table + révoquer) • Devices trustés (table + révoquer) • MFA enrollments (PASSKEY/TOTP/BACKUP_CODES + last_used) • Audit historique (timeline 20 dernières actions concernant ce user).

### 4.4 Octroi de droit — modal stepper 4 steps depuis `/admin/users`
1. **Sélection** : user (auto-rempli ou autocomplete) + radio rôle cible (filtrée par grantor level).
2. **Justification** : textarea ≥ 50 chars, validation.
3. **OTP confirmation** : input 8 chiffres monospace JetBrains Mono, countdown 5min, bouton "Renvoyer" (rate-limit affiché).
4. **Résumé + submit** → `POST /api/admin/users/:id/roles/grant`.
Si le grant requiert dual-control (ADMIN→ADMIN), créer record `admin_role_grants(status=PENDING)`, email approver SUPER-ADMIN avec lien d'approbation OTP-protégé.

### 4.5 Devices trustés — `/admin/devices` (global) + section in detail user
Table : User • Fingerprint (truncated 12 chars) • Type (yubikey/touchid/webauthn/ua-hash) • IP • Created • Last accessed • Trusted-until • Actions.
Modal détail : full fingerprint, UA parse (browser+OS), géoloc IP approximative, history access (10 sessions), revoke.

### 4.6 MFA Enrollment — `/admin/security/mfa` (self) + `/admin/users/:id/mfa` (admin-managed)
3 onglets accordéon :
- **PassKey** : button "Ajouter" → `navigator.credentials.create()` → list table (name, created, last_used, rename, delete).
- **TOTP** : modal stepper (1-Generate→QR+secret base32 copyable, 2-Test 6 chiffres, 3-Confirmé) → list (created, last_used, disable).
- **Backup codes** : auto-générés 10 codes single-use à l'enrollment TOTP, modal "Télécharger .txt / Copier / Imprimer", bouton "Régénérer" (invalide les anciens).

### 4.7 Audit log — `/admin/audit`
- Sidebar sticky filtres : date range picker, autocomplete actor, multi-select action types (`USER_CREATED`, `ROLE_GRANTED`, `OTP_SENT`, `OTP_VERIFIED`, `OTP_FAILED`, `MFA_ENROLLED`, `DEVICE_TRUSTED`, `SESSION_REVOKED`, `BREAK_GLASS_ACTIVATED`, `RECOVERY_CODE_USED`, …), bouton "Apply".
- Timeline verticale, expandable (before/after JSON diff, IP, UA, traceId → href Jaeger), pagination 50.
- Export CSV/JSON (top-right dropdown).

### 4.8 Sessions actives — `/admin/sessions`
Table globale : User • SessionId tronqué • Created • Last active • IP • Device fingerprint • Status. Action `Force logout` → confirm dialog → `DELETE /api/admin/sessions/:id` → publish Redpanda `admin.session.revoked` → ARMAGEDDON invalide → row supprimée live.

### 4.9 Configuration Center — `/admin/settings` (SUPER-ADMIN edit, ADMIN read)

Toute la politique de sécurité doit être **modifiable depuis l'UI**, sans redéploiement. Chaque paramètre :
- est versionné (table `admin_settings` avec colonne `version`),
- déclenche une entrée audit `SETTINGS_UPDATED` (avec `oldValue` / `newValue` / `actorId` / `traceId`),
- est lu via `GET /api/admin/settings` (cache BFF 30s + invalidation sur PUT),
- supporte un bouton "Restaurer la valeur précédente" (rollback à `version - 1`).

Layout : sidebar gauche (catégories) + main (formulaires Material). Chaque catégorie est sa propre carte avec actions `Sauvegarder` / `Annuler` / `Voir l'historique` (timeline modal des changements).

**6 catégories de paramètres** :

#### A. Politique OTP (`otp.*`)
| Clé | Type | Min | Max | Défaut | Description |
|---|---|---|---|---|---|
| `otp.lifetime_seconds` | int | 60 | 900 | 300 | Durée de validité d'un OTP |
| `otp.max_attempts` | int | 1 | 10 | 3 | Tentatives avant lock |
| `otp.lock_duration_seconds` | int | 60 | 3600 | 900 | Durée de lock post-fail |
| `otp.length` | int | 6 | 10 | 8 | Nombre de chiffres |
| `otp.rate_limit_per_user_5min` | int | 1 | 10 | 3 | OTP émis/user/5min |
| `otp.allowed_methods` | enum[] | — | — | `[email,totp]` | Canaux autorisés |

#### B. Device Trust (`device_trust.*`)
| Clé | Type | Min | Max | Défaut |
|---|---|---|---|---|
| `device_trust.enabled` | bool | — | — | `true` |
| `device_trust.ttl_days` | int | 7 | 90 | 30 |
| `device_trust.max_per_user` | int | 1 | 20 | 5 |
| `device_trust.re_verify_on_ip_change` | bool | — | — | `false` |
| `device_trust.fingerprint_strictness` | enum | — | — | `medium` (`low`=UA only / `medium`=UA+IP/24+lang / `high`=+TLS-fp) |
| `device_trust.auto_revoke_on_password_change` | bool | — | — | `true` |

#### C. Sessions (`session.*`)
| Clé | Type | Min | Max | Défaut |
|---|---|---|---|---|
| `session.idle_timeout_minutes` | int | 5 | 480 | 480 |
| `session.absolute_max_minutes` | int | 60 | 1440 | 480 |
| `session.max_concurrent_per_user` | int | 1 | 10 | 3 |
| `session.force_relogin_on_role_change` | bool | — | — | `true` |
| `session.cookie_samesite` | enum | — | — | `lax` (lax/strict/none) |

#### D. MFA & Recovery (`mfa.*`)
| Clé | Type | Défaut |
|---|---|---|
| `mfa.required_for_admin_levels` | enum[] | `[SUPER-ADMIN, ADMIN]` |
| `mfa.required_for_manager` | bool | `false` |
| `mfa.passkey_enabled` | bool | `true` |
| `mfa.totp_enabled` | bool | `true` |
| `mfa.totp_issuer` | text | `"FasoDigitalisation"` |
| `mfa.totp_window` | int (1–3) | `1` |
| `mfa.recovery_codes_count` | int (8–20) | `10` |
| `mfa.recovery_codes_validity_days` | int (90–730) | `365` |

#### E. Octroi de droits & Break-Glass (`grant.*`, `break_glass.*`)
| Clé | Type | Défaut |
|---|---|---|
| `grant.dual_control_for_admin` | bool | `true` |
| `grant.justification_min_length` | int (20–500) | `50` |
| `grant.expiry_default_days` | int (0=permanent–365) | `0` |
| `break_glass.enabled` | bool | `true` |
| `break_glass.ttl_seconds` | int (3600–86400) | `14400` (4h) |
| `break_glass.justification_min_length` | int | `80` |
| `break_glass.notify_all_super_admins` | bool | `true` |
| `break_glass.require_otp` | bool | `true` |

#### F. Audit & rétention (`audit.*`)
| Clé | Type | Défaut |
|---|---|---|
| `audit.retention_days` | int (90–2555) | `2555` (7 ans, conformité Loi 010-2004 BF) |
| `audit.export_csv_enabled` | bool | `true` |
| `audit.export_json_enabled` | bool | `true` |
| `audit.immutable_mode` | bool | `true` (append-only PostgreSQL) |
| `audit.trace_jaeger_link` | text URL template | `http://localhost:16686/trace/{traceId}` |

**Composants UI à designer** :
- `<faso-setting-row>` : clé + label i18n + input typé (toggle / number-spinner / slider / text / multi-select chip / enum radio) + bouton info (tooltip description) + indicator dirty + bouton revert.
- `<faso-setting-history-dialog>` : timeline des versions (qui, quand, ancienne→nouvelle valeur, motif optionnel).
- `<faso-setting-category-card>` : wrapper avec save/cancel/history actions.
- Diff viewer JSON pour changements complexes (enum[] / record).

**Comportement save** :
- Validation Zod côté BFF AVANT persist.
- Confirmation dialog si changement à risque (ex : `device_trust.enabled = false`, `mfa.required_for_admin_levels = []`, `audit.immutable_mode = false`) → "Êtes-vous sûr ?" + champ texte `motif` requis (≥ 20 chars) qui finit dans audit.metadata.
- Envoi `PUT /api/admin/settings` avec `{key, value, version, motif?}`.
- Optimistic concurrency control : si `version` envoyée ≠ version actuelle DB → 409 Conflict + UI affiche "Un autre admin a modifié ce paramètre".
- Publish event Redpanda `admin.settings.changed` (key + old + new + actor) → ARMAGEDDON / KAYA invalident leurs caches.

## 5. Flows utilisateurs critiques

### 5.1 Signup ADMIN par SUPER-ADMIN (invitation)
1. SA `/admin/users` → bouton "Invite admin" → modal email cible.
2. Email envoyée (template invitation) → lien `/auth/signup?role=ADMIN&token=<jwt-invite>`.
3. Cible remplit form (password fort + confirm).
4. **MFA enrollment obligatoire** (PassKey + TOTP).
5. 10 backup codes téléchargés.
6. Redirect `/admin` dashboard.
7. SA reçoit confirmation email + audit `USER_CREATED`.

### 5.2 Login admin (avec device trust skip-OTP)
1. Browser → `:8080/admin` → ARMAGEDDON redirect `/auth/login` si non auth.
2. Email + password → Kratos session.
3. ARMAGEDDON check JWT + Keto.
4. **Si fingerprint UA+IP présent dans KAYA `dev:{userId}:{fp}`** → skip MFA, accès direct.
5. **Sinon** → choisir méthode MFA (PassKey priorité / TOTP / OTP mail / backup code).
6. Si OTP mail : `POST /admin/otp/issue` → publish Redpanda `auth.otp.issue` → notifier consumer → Mailpit/SMTP. Code 8 chiffres affiché.
7. Verify → `POST /admin/otp/verify` → audit + redirect dashboard.

### 5.3 Octroi MANAGER par ADMIN avec dual-control
1. ADMIN → `/admin/users` → search → "Manage roles" → stepper.
2. Steps 1→4 (cf. §4.4).
3. Backend crée `admin_role_grants(status=PENDING)` car ADMIN→ADMIN exige approbation SA. Si MANAGER cible : auto-approuvé.
4. SA reçoit email avec lien approve.
5. Click → OTP confirm → `status=APPROVED`.
6. Cible reçoit email "Rôle MANAGER vous a été octroyé".
7. Au prochain login, capacités MANAGER actives.

### 5.4 Break-Glass 4h
1. ADMIN → `/admin/break-glass` → form (justification ≥ 80 chars + select target capability).
2. OTP 8 chiffres mail confirmation.
3. Submit → KAYA SETEX `auth:break_glass:{userId}` TTL 14400s, Keto tuple temporaire `super_admin@user`.
4. Publish `admin.break_glass.activated` → tous les SA notifiés Slack/email.
5. Audit immutable.
6. À T+4h, auto-révocation + audit `BREAK_GLASS_EXPIRED`.

## 6. Design system

### Couleurs (palette Burkina Faso, accents subtils)
- Primaire : `--color-primary: #1b5e20` (vert foncé BF).
- Secondaire accent : `--color-accent: #FFD700` (or BF) — utilisé sparingly (badges SUPER-ADMIN, focus rings).
- Critique : `--color-danger: #E53935` (rouge BF).
- Success : `--color-success: #4CAF50`.
- Pending : `--color-pending: #FF9800`.
- Fonds : `--color-bg: #FFFFFF` (light) / `#0F1419` (dark) ; `--color-surface: #F5F5F5` / `#1E2429`.
- Mode sombre via `:root[data-theme="dark"]` CSS variables.

### Typographie
- UI : **Inter** (sans-serif, déjà chargée dans poulets-platform).
- Codes OTP / fingerprints / trace IDs : **JetBrains Mono**.
- Échelle : 12 / 14 / 16 / 18 / 24 / 32 px.

### Composants
Material Design 3 customisé via design tokens CSS :
- `--spacing-base: 8px`, scale `4 / 8 / 12 / 16 / 24 / 32 / 48`.
- `--radius-sm: 4px`, `--radius-md: 8px`, `--radius-lg: 16px`.
- `--elevation-1` à `--elevation-5` pour cards/dialogs.
- Modals max-width 600px (default) / 800px (stepper) avec `backdrop-filter: blur(4px)`.

### Accessibilité
- WCAG 2.1 AA. Contraste ≥ 4.5:1 (texte) / ≥ 3:1 (UI).
- Navigation clavier complète (Tab, Shift+Tab, Enter, Escape, Flèches dans tables).
- ARIA `role`, `aria-label`, `aria-describedby`, `aria-live="polite"` sur snackbars.
- Focus rings visibles `outline: 2px solid var(--color-accent); outline-offset: 2px`.

### i18n
ngx-translate. Default `fr-BF`, fallback `en`. Date `dd/MM/yyyy` (FR) / `MM/dd/yyyy` (EN). Timezone `Africa/Ouagadougou` partout. Strings dans `assets/i18n/{fr,en}.json`.

## 7. Modèle de données (TypeScript pour mocks)

```typescript
// shared/models/admin.model.ts
export type AdminLevel = 'SUPER-ADMIN' | 'ADMIN' | 'MANAGER';

export interface AdminUser {
  id: string;
  email: string;
  firstName: string;
  lastName: string;
  phone?: string;
  department?: string;
  role: AdminLevel;
  level: 0 | 1 | 2;
  verified: boolean;
  mfaEnrolled: { passkey: boolean; totp: boolean; backupCodes: number };
  createdAt: string; // ISO 8601
  lastActive: string;
  status: 'active' | 'suspended';
  failedLoginAttempts: number;
  trustedDevicesCount: number;
}

export interface AdminSession {
  id: string;
  userId: string;
  userEmail: string;
  createdAt: string;
  expiresAt: string;
  lastActive: string;
  ipAddress: string;
  userAgent: string;
  deviceFingerprint: string;
  isCurrent: boolean;
}

export interface TrustedDevice {
  id: string;
  userId: string;
  fingerprint: string;
  deviceType: 'yubikey' | 'touchid' | 'windows-hello' | 'webauthn' | 'ua-hash';
  uaString: string;
  ipAddress: string;
  createdAt: string;
  lastAccessed: string;
  trustedUntil: string;
}

export interface MfaEnrollment {
  id: string;
  userId: string;
  type: 'PASSKEY' | 'TOTP' | 'BACKUP_CODES';
  label?: string;
  createdAt: string;
  lastUsed: string | null;
}

export type AuditAction =
  | 'USER_CREATED' | 'USER_SUSPENDED' | 'USER_REACTIVATED'
  | 'ROLE_GRANTED' | 'ROLE_REVOKED'
  | 'OTP_ISSUED' | 'OTP_VERIFIED' | 'OTP_FAILED'
  | 'MFA_ENROLLED' | 'MFA_REMOVED'
  | 'DEVICE_TRUSTED' | 'DEVICE_REVOKED'
  | 'SESSION_REVOKED' | 'PASSWORD_RESET'
  | 'RECOVERY_CODES_GENERATED' | 'RECOVERY_CODE_USED'
  | 'BREAK_GLASS_ACTIVATED' | 'BREAK_GLASS_EXPIRED'
  | 'SETTINGS_UPDATED' | 'SETTINGS_REVERTED';

export type SettingValueType = 'bool' | 'int' | 'text' | 'enum' | 'enum[]' | 'record';
export type SettingCategory = 'otp' | 'device_trust' | 'session' | 'mfa' | 'grant' | 'break_glass' | 'audit';

export interface AdminSetting<T = unknown> {
  key: string;                              // ex: 'otp.lifetime_seconds'
  category: SettingCategory;
  value: T;
  valueType: SettingValueType;
  defaultValue: T;
  minValue?: T;
  maxValue?: T;
  requiredRoleToEdit: AdminLevel;
  version: number;
  updatedAt: string;
  updatedBy: string;
  descriptionI18nKey: string;               // ex: 'admin.settings.otp.lifetime.desc'
}

export interface SettingHistoryEntry {
  id: string;
  key: string;
  version: number;
  oldValue: unknown;
  newValue: unknown;
  motif?: string;
  changedBy: string;
  changedByEmail: string;
  changedAt: string;
  traceId: string;
}

export interface AuditEntry {
  id: string;
  actorId: string;
  actorEmail: string;
  action: AuditAction;
  resourceType: string;
  resourceId: string;
  oldValue?: unknown;
  newValue?: unknown;
  metadata?: Record<string, unknown>;
  ipAddress: string;
  userAgent: string;
  traceId: string;
  createdAt: string;
}

export interface AdminAlert {
  id: string;
  severity: 'info' | 'warning' | 'critical';
  title: string;
  description: string;
  affectedService: string;
  createdAt: string;
  acknowledged: boolean;
}

export interface RoleGrantRequest {
  userId: string;
  targetRole: AdminLevel;
  justification: string;
  otpCode: string; // 8 digits
}
```

## 8. Schéma DB additionnel (PostgreSQL)

À placer dans `poulets-platform/backend/src/main/resources/db/migration/V5__admin_tables.sql` ou équivalent dans `auth-ms`.

```sql
-- Audit log (étend l'existant si V1 contient déjà la base)
ALTER TABLE audit_log
  ADD COLUMN IF NOT EXISTS resource_type VARCHAR(50),
  ADD COLUMN IF NOT EXISTS old_value JSONB,
  ADD COLUMN IF NOT EXISTS new_value JSONB,
  ADD COLUMN IF NOT EXISTS metadata JSONB,
  ADD COLUMN IF NOT EXISTS trace_id VARCHAR(32),
  ADD COLUMN IF NOT EXISTS user_agent TEXT;
CREATE INDEX IF NOT EXISTS idx_audit_log_action_time ON audit_log(action, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_actor ON audit_log(actor_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_target ON audit_log(target_type, target_id);

-- Recovery codes (single-use)
CREATE TABLE IF NOT EXISTS recovery_codes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  code_hash VARCHAR(255) NOT NULL UNIQUE, -- bcrypt(code + salt)
  used_at TIMESTAMPTZ,
  generated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '1 year')
);
CREATE INDEX idx_recovery_codes_user_unused ON recovery_codes(user_id) WHERE used_at IS NULL;

-- Device registrations (WebAuthn + ua-hash)
CREATE TABLE IF NOT EXISTS device_registrations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  fingerprint VARCHAR(255) NOT NULL,
  device_type VARCHAR(50) NOT NULL,
  public_key_pem TEXT,            -- WebAuthn only
  ua_string TEXT,
  ip_address VARCHAR(45),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_used_at TIMESTAMPTZ,
  trusted_at TIMESTAMPTZ DEFAULT now(),
  revoked_at TIMESTAMPTZ,
  CONSTRAINT uq_device_user_fp UNIQUE(user_id, fingerprint)
);

-- TOTP enrollments (secret AES-256-GCM)
CREATE TABLE IF NOT EXISTS totp_enrollments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
  secret_encrypted VARCHAR(500) NOT NULL,
  enrolled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  disabled_at TIMESTAMPTZ
);

-- MFA status tracking
CREATE TABLE IF NOT EXISTS mfa_status (
  user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  totp_enabled BOOLEAN DEFAULT false,
  passkey_count INTEGER DEFAULT 0,
  backup_codes_remaining INTEGER DEFAULT 0,
  trusted_devices_count INTEGER DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Role grants (workflow dual-control)
CREATE TABLE IF NOT EXISTS admin_role_grants (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  grantor_id UUID NOT NULL REFERENCES users(id),
  grantee_id UUID NOT NULL REFERENCES users(id),
  role_id UUID NOT NULL REFERENCES roles(id),
  justification TEXT NOT NULL,
  status VARCHAR(20) NOT NULL DEFAULT 'PENDING',  -- PENDING | APPROVED | REJECTED | EXPIRED
  approver_id UUID REFERENCES users(id),
  expires_at TIMESTAMPTZ,                          -- pour Break-Glass
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  approved_at TIMESTAMPTZ,
  CHECK (status IN ('PENDING','APPROVED','REJECTED','EXPIRED'))
);
CREATE INDEX idx_role_grants_pending ON admin_role_grants(status) WHERE status = 'PENDING';

-- Settings sécurité (versionné, optimistic concurrency)
CREATE TABLE IF NOT EXISTS admin_settings (
  key VARCHAR(80) PRIMARY KEY,
  value JSONB NOT NULL,
  value_type VARCHAR(20) NOT NULL,        -- bool | int | text | enum | enum[] | record
  category VARCHAR(40) NOT NULL,          -- otp | device_trust | session | mfa | grant | break_glass | audit
  description_i18n_key VARCHAR(120),
  min_value JSONB,
  max_value JSONB,
  default_value JSONB NOT NULL,
  required_role_to_edit VARCHAR(20) NOT NULL DEFAULT 'SUPER-ADMIN',
  version INTEGER NOT NULL DEFAULT 1,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_by UUID REFERENCES users(id)
);

-- Historique des changements (versions précédentes pour rollback + audit)
CREATE TABLE IF NOT EXISTS admin_settings_history (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  key VARCHAR(80) NOT NULL REFERENCES admin_settings(key),
  version INTEGER NOT NULL,
  old_value JSONB,
  new_value JSONB NOT NULL,
  motif TEXT,                              -- justif obligatoire pour changements à risque
  changed_by UUID REFERENCES users(id),
  changed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  trace_id VARCHAR(32),
  CONSTRAINT uq_settings_history_key_version UNIQUE(key, version)
);
CREATE INDEX idx_settings_history_changed_at ON admin_settings_history(changed_at DESC);
```

## 9. Schéma Redpanda topics (Avro)

À créer via `rpk topic create` dans `INFRA/scripts/redpanda-init.sh` :

```yaml
# auth.otp.issue — partitions=3, retention=7d
OtpIssueEnvelope:
  userId: UUID
  otpId: UUID
  method: enum[email, sms, totp]
  channel: string
  issuedAt: long
  issuedBy: UUID  # admin actor si grant-flow
  expiresAt: long
  traceId: string

# auth.otp.verified — partitions=3, retention=30d
# auth.role.granted — partitions=1, retention=90d
# auth.device.trusted — partitions=3, retention=30d
# auth.session.revoked — partitions=3, retention=7d
# admin.break_glass.activated — partitions=1, retention=365d
# admin.settings.changed — partitions=1, retention=2555d (7 ans, Loi 010-2004 BF)
SettingsChangedEnvelope:
  key: string
  oldValue: bytes (JSON)
  newValue: bytes (JSON)
  oldVersion: int
  newVersion: int
  motif: string?
  changedBy: UUID
  changedAt: long
  traceId: string
```

Naming convention FASO : préfixe `auth.*` pour flows auth, `admin.*` pour actions admin métier.

## 10. Schéma KAYA (clés Redis-compat)

```
# OTP
auth:otp:{otpId}                     HASH  TTL=300s   {code, userId, method, expiresAt, attempts, verified}
auth:otp:rl:{userId}                 STR   TTL=300s   counter (max 3)
auth:otp:lock:{userId}               STR   TTL=900s   marker post-3-fails

# Device trust
dev:{userId}:{fingerprintHash}       HASH  TTL=2592000s (30d)  {createdAt, trustedAt, lastUsedAt, ua, ip, deviceType}

# Recovery codes meta
auth:recovery:{userId}               HASH  TTL=31536000s       {count, generatedAt, lastUsed}

# TOTP enrollment temporaire
auth:totp:temp:{userId}              HASH  TTL=600s            {secret, backupCodes, createdAt}

# Sessions (déjà existant)
auth:sessions:{userId}               ZSET  TTL=28800s          membres = sessionId, score = epoch

# JTI blacklist (déjà existant)
auth:jti:blacklist:{jti}             STR   TTL=jwt_exp

# Break-Glass
auth:break_glass:{userId}            HASH  TTL=14400s (4h)     {justification, activatedAt, expiresAt, otpProof}
```

Fingerprint computation : `SHA-256(UA_normalized + "::" + IP_class_C + "::" + Accept-Language)` puis tronqué 32 hex.

## 11. Endpoints REST attendus

### Frontend → BFF Next.js
- `GET /api/admin/dashboard` → KPIs.
- `GET /api/admin/users?role&department&page&size&sort` → liste paginée.
- `POST /api/admin/users/invite` → invitation email.
- `GET /api/admin/users/:userId` → détail.
- `POST /api/admin/users/:userId/roles/grant` → workflow dual-control.
- `POST /api/admin/users/:userId/roles/revoke`.
- `POST /api/admin/users/:userId/suspend`, `DELETE …/suspend`.
- `GET /api/admin/sessions`, `DELETE /api/admin/sessions/:id`.
- `GET /api/admin/devices`, `POST /api/admin/devices/:id/trust`, `DELETE /api/admin/devices/:id`.
- `POST /api/admin/devices/register/begin`, `POST …/finish`.
- `GET /api/admin/audit?from&to&actor&action&page&size`.
- `GET /api/admin/audit/:id`.
- `POST /api/admin/otp/issue`, `POST /api/admin/otp/verify`.
- `POST /api/admin/totp/enroll/begin`, `POST …/finish`, `DELETE /api/admin/totp`.
- `POST /api/admin/recovery-codes/generate`, `POST /api/admin/recovery-codes/use`.
- `POST /api/admin/break-glass/activate`.
- `GET /api/admin/settings` → `{ category: { key: { value, version, min, max, default, type, requiredRole } } }`.
- `GET /api/admin/settings/:key` → détail.
- `PUT /api/admin/settings/:key` body `{ value, version, motif? }` → 200 ou 409 si version stale.
- `GET /api/admin/settings/:key/history` → liste versions (paginée).
- `POST /api/admin/settings/:key/revert` body `{ targetVersion, motif }` → restaure.

### BFF → Backend (auth-ms / poulets-api via ARMAGEDDON :8080)
Tous les endpoints précédents proxient vers les services backend correspondants en propageant `traceparent` + JWT.

## 12. Contraintes techniques pour le code généré

### Frontend Angular 21
- **Standalone components uniquement** (pas de NgModule). Pas de `*ngIf`/`*ngFor` — `@if @for @switch` natifs.
- `ChangeDetectionStrategy.OnPush` partout. Données via `signal()`, `computed()`, `input()`, `output()`.
- HTTP `HttpClient` avec interceptor `authInterceptor` (Bearer JWT depuis `AuthService`).
- Routes guards : `authGuard`, `adminGuard`, `roleGuard('SUPER-ADMIN' | 'ADMIN' | 'MANAGER')`.
- Reactive Forms typés (`FormGroup<{...}>`).
- Lazy-load chaque feature admin via `loadComponent`.
- OTel SDK browser injecté dans `main.ts` avec `B3MultiPropagator` ou `W3CTraceContextPropagator`.
- Header SPDX : `// SPDX-License-Identifier: AGPL-3.0-or-later` en première ligne de chaque `.ts`/`.html`/`.scss`.

### BFF Next.js 16
- App Router, `app/api/admin/.../route.ts` GET/POST handlers.
- Schemas Zod dans `lib/schemas/admin.ts`.
- Middleware `middleware.ts` : check Kratos session + JWT + Keto.
- Server Actions pour mutations (Form actions admin).
- `instrumentation.ts` pour OTel auto-instrumentation Node SDK.
- Error boundaries `error.tsx` par segment.
- SPDX header sur chaque fichier source.
- Fetch upstream avec `cache: 'no-store'` pour endpoints admin.

### Sécurité (anti-patterns à proscrire)
- ❌ JWT/session dans `localStorage`. ✅ Cookies httpOnly + SameSite=Lax (déjà fait par Kratos).
- ❌ Authorization client-only. ✅ Toujours re-vérifier côté BFF + backend + Keto.
- ❌ Audit fire-and-forget. ✅ Producer Redpanda avec retry + DLQ + idempotency key.
- ❌ RBAC sans scope. ✅ Toute capacité a un scope (`tenant_id`, `department_id`, `region_id`).
- ❌ Mock OTP en dev. ✅ Mailpit local avec vraie boucle SMTP, identique à prod.
- ❌ Mention de Redis/Envoy/Istio dans le code généré.

## 13. Tests E2E à viser (cf. `INFRA/docs/PLAYWRIGHT-FULLSTACK-E2E-GUIDE.md`)

Suite cible `poulets-platform/e2e/tests/18-admin-workflows/` :
1. `admin-signup-super-admin.spec.ts` (P99 ≤ 30s)
2. `admin-signup-admin.spec.ts` (invite + OTP)
3. `admin-signup-manager.spec.ts`
4. `admin-login-otp-mail.spec.ts` (P99 ≤ 8s)
5. `admin-login-passkey.spec.ts` (P99 ≤ 3s)
6. `admin-login-totp.spec.ts` (P99 ≤ 5s)
7. `admin-login-recovery-code.spec.ts` (P99 ≤ 4s)
8. `admin-device-trust-skip-otp.spec.ts` (P99 ≤ 12s) — **assertion clé** : `expect(otpInput).not.toBeVisible()` au 2ᵉ login.
9. `admin-grant-role.spec.ts` (workflow dual-control complet)
10. `admin-revoke-role.spec.ts`
11. `admin-audit-query.spec.ts`
12. `admin-session-force-logout.spec.ts`
13. `admin-break-glass.spec.ts` (TTL 4h + auto-révocation)
14. `admin-settings-update.spec.ts` (changer `otp.lifetime_seconds` 300→600 + rollback + version conflict 409)
15. `admin-settings-effect-runtime.spec.ts` (modifier `otp.length` à 6 → vérifier nouvel OTP émis a 6 chiffres immédiatement, sans redéploiement)

Fixtures à enrichir dans `poulets-platform/e2e/fixtures/` :
- `actors.ts` : ajouter rôle `SUPER-ADMIN`, structures `mfaMethod`, `trustedDevices`, `recoveryCode`.
- `mailpit.ts` : `waitForOtp(email, { regex: /\b(\d{8})\b/ })` (8 chiffres).
- `session.ts` : `loginWithOtp()`, `loginWithPasskey()`, `loginWithTotp()`, `loginWithRecoveryCode()`.
- Nouveau `device-trust.ts`.
- Nouveau page object `AdminDashboardPage.ts`.

## 14. Livrable attendu de Claude Design

### Tree Angular
```
poulets-platform/frontend/src/app/features/admin/
├── routes.ts (mis à jour avec lazy-load)
├── pages/
│   ├── dashboard/admin-dashboard.component.{ts,html,scss}
│   ├── users/admin-users.component.{ts,html,scss}
│   ├── users-detail/admin-user-detail.component.{ts,html,scss}
│   ├── devices/admin-devices.component.{ts,html,scss}
│   ├── mfa/admin-mfa.component.{ts,html,scss}
│   ├── audit/admin-audit.component.{ts,html,scss}
│   ├── sessions/admin-sessions.component.{ts,html,scss}
│   ├── settings/
│   │   ├── admin-settings.component.{ts,html,scss}             # layout + sidebar catégories
│   │   ├── settings-otp.component.*
│   │   ├── settings-device-trust.component.*
│   │   ├── settings-session.component.*
│   │   ├── settings-mfa.component.*
│   │   ├── settings-grant-break-glass.component.*
│   │   └── settings-audit.component.*
│   └── break-glass/admin-break-glass.component.{ts,html,scss}
├── components/
│   ├── grant-role-stepper.component.*
│   ├── otp-confirm.component.*
│   ├── webauthn-enroll.component.*
│   ├── totp-enroll.component.*
│   ├── backup-codes-dialog.component.*
│   ├── session-detail.component.*
│   ├── audit-timeline.component.*
│   ├── role-badge.component.*           # chip coloré par level
│   ├── trace-link.component.*           # href Jaeger
│   ├── setting-row.component.*          # input typé + revert + history
│   ├── setting-category-card.component.*
│   └── setting-history-dialog.component.*
├── services/
│   ├── admin-user.service.ts
│   ├── admin-audit.service.ts
│   ├── admin-session.service.ts
│   ├── admin-device.service.ts
│   ├── admin-mfa.service.ts
│   ├── admin-otp.service.ts
│   ├── admin-settings.service.ts
│   └── admin-break-glass.service.ts
├── guards/
│   ├── super-admin.guard.ts
│   └── admin-level.guard.ts             # roleGuard avec niveau min
├── models/admin.model.ts
└── tokens/design-tokens.scss            # CSS vars couleurs/spacing/typo
```

### Tree BFF Next.js
```
poulets-platform/bff/src/app/api/admin/
├── dashboard/route.ts
├── users/
│   ├── route.ts                    # GET list, POST invite
│   ├── [userId]/
│   │   ├── route.ts                # GET detail
│   │   ├── roles/grant/route.ts
│   │   ├── roles/revoke/route.ts
│   │   ├── suspend/route.ts
│   │   ├── mfa/route.ts
│   │   └── sessions/route.ts
├── sessions/
│   ├── route.ts
│   └── [sessionId]/route.ts
├── devices/
│   ├── route.ts
│   ├── register/begin/route.ts
│   └── register/finish/route.ts
├── audit/
│   ├── route.ts
│   └── [id]/route.ts
├── otp/
│   ├── issue/route.ts
│   └── verify/route.ts
├── totp/
│   ├── enroll/begin/route.ts
│   └── enroll/finish/route.ts
├── recovery-codes/
│   ├── generate/route.ts
│   └── use/route.ts
├── break-glass/activate/route.ts
└── settings/
    ├── route.ts                              # GET all
    └── [key]/
        ├── route.ts                          # GET / PUT
        ├── history/route.ts                  # GET versions
        └── revert/route.ts                   # POST
```

### Fichiers support
- `lib/admin-auth.ts` (middleware authz check Keto + JWT).
- `lib/admin-otp.ts` (helpers).
- `lib/admin-audit.ts` (publish Redpanda + audit_log INSERT).
- `lib/schemas/admin.ts` (Zod schemas).
- `app/admin/loading.tsx`, `app/admin/error.tsx`.

### Storybook (optionnel mais souhaité)
- Stories par composant atomique (`role-badge`, `audit-timeline`, `otp-confirm`, etc.).
- Variants Light + Dark.

### README
- Section "Installation" : où placer chaque fichier dans `poulets-platform/`.
- Section "DB migrations" : référence à `V5__admin_tables.sql`.
- Section "Redpanda topics" : commande `rpk topic create …`.
- Section "Variables d'env BFF" : `KRATOS_PUBLIC_URL`, `AUTH_MS_URL`, `KETO_READ_URL`, `KETO_WRITE_URL`, `REDPANDA_BROKERS`, `MAILPIT_API_URL`.
- Section "Tests E2E" : référence à la suite `18-admin-workflows/`.

## 15. Header obligatoire sur chaque fichier source

```typescript
// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso
```

## 16. Récapitulatif contraintes critiques (TL;DR pour Claude Design)

1. **Hiérarchie** : SUPER-ADMIN ⊃ ADMIN ⊃ MANAGER, transitive.
2. **OTP** : 8 chiffres numériques, TTL 5min, 3 tentatives max.
3. **MFA** : PassKey (priorité) + TOTP + 10 backup codes single-use.
4. **Device-trust** : KAYA TTL 30 jours, fingerprint UA+IP+Accept-Language.
5. **Bus** : Redpanda topics `auth.otp.*` / `admin.role.granted` / `admin.device.trusted` / `admin.session.revoked` / `admin.break_glass.activated`.
6. **Mail dev** : Mailpit :1025 SMTP / :8025 API.
7. **Gateway** : ARMAGEDDON :8080 (jamais d'appel direct backend depuis browser).
8. **i18n** : FR-BF default, EN fallback.
9. **Souveraineté** : INTERDIT Redis/Envoy/Istio/Okta/Auth0/AWS/GCP/Azure.
10. **Licence** : AGPL-3.0-or-later, header SPDX en tête de chaque fichier.
11. **Stack frontend** : Angular 21 standalone + signals + Material Design 3 + OTel browser.
12. **BFF** : Next.js 16 App Router + Zod + jose + OTel Node.
13. **A11y** : WCAG 2.1 AA stricte.
14. **Tests** : 13 specs E2E ciblés dans `18-admin-workflows/`.
15. **Configuration Center** : 6 catégories de settings tous éditables via UI (`/admin/settings`), versionnés en DB (`admin_settings` + `admin_settings_history`), publiés sur Redpanda `admin.settings.changed`, avec rollback + optimistic concurrency. SUPER-ADMIN edit, ADMIN read.

---

*Brief consolidé issu de 4 explorations parallèles : (1) frontend Angular local + BFF, (2) repos GitHub fasodigit (e-ticket / ALT-MISSION / E-SCHOOL / Etat-civil), (3) backend gap analysis auth-ms+KAYA+Kratos+Keto+Redpanda, (4) Playwright E2E patterns. Prêt pour Claude Design.*
