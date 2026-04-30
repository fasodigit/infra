<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Delta requirements admin-UI v2 — durcissement RBAC + protection SUPER-ADMIN + recovery -->

# Delta requirements — Admin-UI v2 (2026-04-30)

Ce document **AMENDE** le brief Claude Design (`CLAUDE-DESIGN-PROMPT-ADMIN-UI.md`) et le gap analysis Phase 4.a (`GAP-ANALYSIS-PHASE-4A.md`). Il capture des contraintes énoncées **après le lancement de la Phase 4.b**. Toutes les décisions ci-dessous **priment** sur les versions antérieures.

## 1. Différentiation des autorisations entre comptes même rôle

### Règle
> **Deux comptes ADMIN ne peuvent JAMAIS partager exactement le même ensemble d'autorisations.** Idem pour deux comptes MANAGER. Le rôle (ADMIN, MANAGER) définit un *périmètre maximal* ; chaque compte reçoit un **sous-ensemble distinct** de capacités à l'octroi.

Le rôle SUPER-ADMIN est l'exception : tous les SUPER-ADMIN ont la totalité des capacités.

### Modèle de données
- **Capability registry** (statique, Java enum + i18n) : ~30 capacités fines (`users:invite`, `users:suspend`, `users:manage:dept:X`, `roles:grant_admin`, `roles:grant_manager`, `roles:revoke`, `sessions:list`, `sessions:revoke`, `devices:list`, `devices:revoke`, `mfa:reset`, `audit:view`, `audit:export`, `settings:read`, `settings:write_otp`, `settings:write_device_trust`, `settings:write_session`, `settings:write_mfa`, `settings:write_grant`, `settings:write_break_glass`, `settings:write_audit`, `break_glass:activate`, `recovery:initiate`, `recovery:complete`, …).
- **Migration V10** : `account_capability_grants(id UUID, user_id UUID, capability_key VARCHAR(80), scope JSONB, granted_by UUID, granted_at TIMESTAMPTZ, revoked_at TIMESTAMPTZ, granted_for_role VARCHAR(20))` + index `(user_id) WHERE revoked_at IS NULL`.
- **Keto** : chaque capacité = relation dans un namespace `Capability`. Tuple par user : `Capability:platform#<key>@<userId>`. Le namespace `AdminRole` (super_admin/admin/manager) reste, mais les checks fins de capacité passent désormais par `Capability`.
- **Contrainte uniqueness (soft, UI level)** : à l'octroi d'un nouveau rôle ADMIN ou MANAGER, le BFF appelle `/api/admin/capabilities/check-uniqueness?role=ADMIN&caps=...` qui retourne un warning si un autre compte a exactement le même set. **Pas de hard fail DB** — c'est un signal pour l'UI ("Cet ensemble est identique à celui de jane@faso.bf — préférez ajouter ou retirer ≥1 capacité"). Le SUPER-ADMIN peut forcer.

### UI impact (Configuration Center + Grant Role stepper)
- **Step 1bis du stepper grant-role** (entre "Sélection rôle" et "Justification") : sélection multi-checkbox des capacités (groupées par domaine : Users / Sessions / Devices / MFA / Audit / Settings / Break-Glass).
- **Page user-detail** : affiche le set de capacités effectif + bouton "Modifier les capacités" (re-grant flow).

## 2. Protection SUPER-ADMIN

### Invariants stricts
1. **Indépression** : un compte avec rôle `SUPER-ADMIN` **ne peut pas être supprimé** (DELETE rejeté).
2. **In-suspendable** : un compte SUPER-ADMIN ne peut pas être suspendu (`POST /admin/users/:id/suspend` → 403).
3. **In-démouvable** : on ne peut pas révoquer le rôle SUPER-ADMIN d'un compte si c'est le DERNIER SUPER-ADMIN actif (`COUNT(*) WHERE role = 'SUPER-ADMIN' AND status = 'active' = 1` → 409 Conflict).
4. **In-modifiable rôle** : seul un autre SUPER-ADMIN peut modifier les capacités d'un SUPER-ADMIN.

### Mise en œuvre
- **DB trigger PostgreSQL** `prevent_super_admin_delete` (V11) : `BEFORE DELETE ON users` + `BEFORE UPDATE` qui RAISE EXCEPTION si target est SUPER-ADMIN et opération destructive.
- **Service guard** dans `AdminUserService.delete()`, `.suspend()`, `.demoteRole()` : check + retourne `403` ou `409`.
- **Invariant `LAST_SUPER_ADMIN_PROTECTION`** : avant tout opération de retrait, comptage `SELECT COUNT(*) FROM users JOIN user_roles ON … WHERE role.name = 'SUPER-ADMIN' AND user.status = 'active'`. Si == 1 et target = ce dernier → reject.
- **Audit immutable** : toute tentative bloquée → entrée `audit_log` avec `action = 'SUPER_ADMIN_PROTECTION_TRIGGERED'`.

## 3. Self-management SUPER-ADMIN (et tous niveaux)

Le compte SUPER-ADMIN doit pouvoir **gérer ses propres facteurs d'auth** depuis l'admin-UI sans intervention tierce.

### Endpoints à exposer (s'ajoutent au gap doc §1)
| Méthode | Chemin | Body | Réponse | Notes |
|---|---|---|---|---|
| POST | `/api/admin/me/password` | `{ currentPassword, newPassword }` | `{ changedAt }` | Proxy → Kratos `/self-service/settings` flow type=password |
| POST | `/api/admin/me/passkeys/enroll/begin` | — | `{ challengeSessionId, challenge, timeout }` | Variante self de `/admin/users/:id/passkeys/enroll/begin` |
| POST | `/api/admin/me/passkeys/enroll/finish` | `{ challengeSessionId, credential }` | `{ passkeyId, enrolledAt }` | — |
| DELETE | `/api/admin/me/passkeys/:passkeyId` | — | `{ deletedAt }` | — |
| POST | `/api/admin/me/totp/enroll/begin` | — | `{ tempSecret, qrCodeUrl, expiresAt }` | — |
| POST | `/api/admin/me/totp/enroll/finish` | `{ code, tempSecret }` | `{ enrolledAt, backupCodes[] }` | — |
| DELETE | `/api/admin/me/totp` | — | `{ disabledAt }` | — |
| POST | `/api/admin/me/recovery-codes/regenerate` | `{ motif: string }` | `{ codes: string[], generatedAt, expiresAt }` | Invalide les anciens codes |
| POST | `/api/admin/me/recovery-codes/use` | `{ code }` | `{ remaining }` | Pour usage en login (cf. §4) |

**Règle** : `userId` extrait du JWT, **pas** du path. Le SUPER-ADMIN qui s'auto-modifie ne déclenche **pas** le workflow dual-control.

### UI impact
- Nouvelle page `/admin/me/security` (alias `/admin/profile/security`) : 5 cards (Mot de passe / PassKey / TOTP / Recovery codes / Sessions actives). Réutilise les composants existants (`webauthn-enroll`, `totp-enroll`, `backup-codes-dialog`).
- Item de sidebar "Mon compte" en bas (avant le footer SUPER-ADMIN actuel).

## 4. Recovery code utilisable au login

### Règle
> Les **codes à usage unique** (recovery codes) **DOIVENT** pouvoir être utilisés comme **second facteur d'authentification** au login lorsque l'utilisateur a perdu son device PassKey ou TOTP. Chaque code est utilisable **une seule fois** ; après utilisation il est marqué consommé en DB.

### Flux login avec recovery code
1. User entre email + password (Kratos `/self-service/login` method=password).
2. AAL2 prompt : choix méthode MFA — `[PassKey] [TOTP] [Code de récupération]`.
3. User clique "Code de récupération" → champ texte (format `XXXX-XXXX`).
4. POST `/api/admin/auth/login/recovery-code` `{ kratosFlowId, code }`.
5. Backend (`auth-ms`) :
   - lookup hash bcrypt dans `recovery_codes` WHERE `user_id` matches, `used_at IS NULL`, `expires_at > now()`.
   - si match → `UPDATE used_at = now()` + audit `RECOVERY_CODE_USED` + publish `auth.recovery.used` topic.
   - retourne 200 + Kratos session AAL2 OK.
   - si pas de match → 403 + audit `RECOVERY_CODE_INVALID`.
   - si user a 0 code restant → mail SUPER-ADMIN "user X has used last recovery code".
6. UI redirige vers dashboard.

### Garde rail
- Si `recovery_codes_remaining = 0` post-use → bannière persistante "Régénérez vos codes" jusqu'à action.
- Limite : max 10 tentatives échouées sur recovery code en 1h → lock compte 1h (KAYA `auth:recovery:lock:{userId}` TTL 3600s).

## 5. Account Recovery (perte totale d'accès)

Quand un user a perdu **tous ses facteurs MFA** (device + recovery codes), 2 chemins :

### A. Self-recovery (utilisateur initie)
1. Page login → lien "J'ai perdu l'accès à mon compte".
2. User entre email → `POST /api/admin/auth/recovery/initiate` `{ email }`.
3. Backend vérifie email valide + envoie email avec **lien magique signé** (JWT court 30min, single-use, scope=recovery).
4. User clique le lien → page `/auth/recovery?token=...`.
5. Vérifie token → demande **réauthentification email OTP 8 chiffres** (envoyé à l'email).
6. User entre OTP → token consommé → user obtient session AAL1 (degraded).
7. UI **force re-enrollment MFA** (TOTP ou PassKey) avant tout autre action.
8. Audit immutable `ACCOUNT_RECOVERY_SELF_INITIATED` + `ACCOUNT_RECOVERY_COMPLETED`.

### B. Admin-initiated recovery (SUPER-ADMIN aide)
1. SUPER-ADMIN page user-detail → bouton "Lancer la récupération de compte".
2. Modal : motif obligatoire (≥ 50 chars) + OTP confirmation du SUPER-ADMIN.
3. `POST /api/admin/users/:id/recovery/initiate` :
   - reset MFA (delete TotpEnrollment, DeviceRegistrations, RecoveryCodes du user cible).
   - génère un **token de récupération à 8 chiffres** (SecureRandom).
   - publie `auth.recovery.admin_initiated` Redpanda → notifier-ms envoie email user cible avec le token.
   - user dispose de 1h pour utiliser le token.
4. User cible reçoit email avec token + lien `/auth/recovery?adminToken=...`.
5. User entre email + token 8 chiffres → session AAL1.
6. Force re-enrollment MFA.
7. Audit `ACCOUNT_RECOVERY_ADMIN_INITIATED` + `ACCOUNT_RECOVERY_COMPLETED`.

### Migration V12
```sql
CREATE TABLE account_recovery_requests (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id),
  initiated_by UUID REFERENCES users(id),  -- NULL si self
  recovery_type VARCHAR(20) NOT NULL,      -- 'SELF' | 'ADMIN_INITIATED'
  token_hash VARCHAR(255) NOT NULL UNIQUE,
  motif TEXT,
  status VARCHAR(20) NOT NULL DEFAULT 'PENDING',  -- PENDING | USED | EXPIRED | REJECTED
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  used_at TIMESTAMPTZ,
  expires_at TIMESTAMPTZ NOT NULL,
  trace_id VARCHAR(32)
);
CREATE INDEX idx_recovery_pending ON account_recovery_requests(user_id, status) WHERE status = 'PENDING';
```

## 6. Tests E2E à AJOUTER (Phase 4.c — extension du plan §13)

Les **13 specs initiales** restent. **Ajouts obligatoires** :

| # | Spec | Acteur | Vérification clé |
|---|---|---|---|
| 14 | `admin-settings-update.spec.ts` | SUPER-ADMIN | (déjà prévu) |
| 15 | `admin-settings-effect-runtime.spec.ts` | SUPER-ADMIN | (déjà prévu) |
| 16 | `admin-recovery-code-actually-works-at-login.spec.ts` | ADMIN | Génère codes → logout → login email+pwd → choisit "code de récupération" → entre code → **assert dashboard atteint** ; tente le MÊME code une 2ᵉ fois → **assert 403** |
| 17 | `admin-self-recovery-flow.spec.ts` | MANAGER | Lock factors → /auth/recovery → email magic link → OTP 8 chiffres Mailpit → AAL1 → force re-enrollment TOTP → login normal |
| 18 | `admin-admin-initiated-recovery.spec.ts` | SUPER-ADMIN + cible | SA reset MFA cible + token → email Mailpit → cible se logue avec token → re-enrollment forcé |
| 19 | `admin-super-admin-undeletable.spec.ts` | SUPER-ADMIN tente delete autre SA OU dernier SA | Assert 403/409 + audit `SUPER_ADMIN_PROTECTION_TRIGGERED` |
| 20 | `admin-super-admin-self-management.spec.ts` | SUPER-ADMIN | Change password Kratos → enroll new PassKey → enroll new TOTP → regenerate recovery codes → relogin avec nouveau facteur |
| 21 | `admin-granular-capabilities.spec.ts` | SUPER-ADMIN crée 2 ADMINs A et B avec sets différents | A peut suspendre user X mais pas Y, B peut faire l'inverse → assert 403 sur opérations hors capacités |
| 22 | `admin-grant-warns-on-duplicate-capabilities.spec.ts` | SUPER-ADMIN | Tente d'octroyer à C exactement le set de A → UI affiche warning soft, SA force, action passe avec audit `CAPABILITY_SET_DUPLICATE_OVERRIDE` |

## 7. Plan d'amendement (post-Stream A et D1)

À exécuter **dès que les streams A (auth-ms) et D1 (ARMAGEDDON) finissent** :

### Amendment auth-ms (Stream A.2)
1. Ajouter migration V10 (capability_grants) + V11 (trigger SA protection) + V12 (recovery_requests).
2. Créer `service/admin/CapabilityRegistry.java` (enum) + `CapabilityService` + `AccountRecoveryService`.
3. Étendre `AdminUserController` : guards SUPER-ADMIN protection sur DELETE/SUSPEND/DEMOTE.
4. Créer `AdminMeController` (`/admin/me/*`).
5. Étendre `AdminRoleGrantService` : workflow grant inclut `capabilities: string[]`.
6. Ajouter endpoint `GET /admin/capabilities` + `POST /admin/capabilities/check-uniqueness`.
7. Ajouter `POST /admin/auth/login/recovery-code`.
8. Ajouter `POST /admin/users/:id/recovery/initiate` + `POST /admin/auth/recovery/initiate`.
9. Topics Redpanda nouveaux : `auth.recovery.used`, `auth.recovery.admin_initiated`, `auth.recovery.completed`.

### Amendment BFF (Stream C.2)
1. 9 nouvelles routes Next.js `/api/admin/me/*`.
2. 3 nouvelles routes recovery (initiate self / initiate admin / use recovery code at login).
3. 1 route capabilities (list + check-uniqueness).
4. Helper Zod : `CapabilitySetSchema`, `RecoveryInitiateSchema`, `RecoveryUseSchema`.

### Amendment frontend Angular (Stream UI delta)
1. Nouvelle page `pages-v2/me-security.page.ts` (5 cards self-management).
2. `grant-role-stepper.dialog.ts` : ajout step 1bis (sélection capacités multi-checkbox).
3. `user-detail.page.ts` : section "Capacités effectives" + bouton "Modifier".
4. `audit.page.ts` : ajout `SUPER_ADMIN_PROTECTION_TRIGGERED` et `CAPABILITY_SET_DUPLICATE_OVERRIDE` dans la liste actions.
5. Ajout route `/auth/recovery` (page publique, hors shell admin).

### Amendment notifier-ms (Stream B.2)
1. 3 templates Handlebars : `admin-recovery-self-link.hbs`, `admin-recovery-admin-token.hbs`, `admin-recovery-completed.hbs`.
2. Consumer `auth.recovery.admin_initiated` → envoie token email au user cible.

### Amendment Keto (D2.2)
1. Namespace `Capability` à ajouter dans `namespaces.ts`.
2. Script seed pour les capacités par défaut sur les SUPER-ADMIN.

## 8. Ordre d'exécution

Phase 4.b reste en cours. À la complétion des streams A + D1, je :

1. Lance **Stream A.2** (amendment auth-ms) — délégué `general-purpose`.
2. Lance en parallèle **Stream C.2** (amendment BFF) + **Stream B.2** (notifier templates).
3. Lance **UI delta** (frontend Angular) — me-security page + capabilities step.
4. Phase 4.c démarre avec les **22 specs E2E** (13 + 9).
5. Phase 4.d cycle-fix.

---

*Delta requirements verrouillé 2026-04-30. Ces contraintes priment.*
