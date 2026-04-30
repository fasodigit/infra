<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Architecture sécurité complète admin-UI v2 — module catalog + journées utilisateur + mapping Playwright -->

# Architecture sécurité admin-UI FASO — schéma complet

**Date** : 2026-04-30
**Scope** : tous les modules de sécurité implémentés au cours des Phases 4.b core + 4.b.2 amendments + 4.b.3 → 4.b.7 hardening.

---

## Section 1 — Catalogue des 23 modules de sécurité

| # | Module | Phase | Couche | Storage | Déclencheur |
|---|---|---|---|---|---|
| M01 | **Argon2id password hashing** | 4.b.3 | auth-ms | Postgres `users.password_hash` + Vault pepper | Création/modif mot de passe + lazy re-hash login |
| M02 | **HMAC + Argon2id OTP hashing** | 4.b.3 | auth-ms | Postgres `admin_otp_requests.otp_hash` + Vault pepper | Émission OTP (issue) |
| M03 | **HMAC + Argon2id recovery codes** | 4.b.3 | auth-ms | Postgres `recovery_codes.code_hash` + Vault pepper | Génération 10 codes |
| M04 | **AES-256-GCM TOTP secret at-rest** | 4.b core | auth-ms | Postgres `totp_enrollments.secret_encrypted` | Enrôlement TOTP |
| M05 | **JWT ES384 signing + JWKS** | existant | auth-ms | DB `jwt_signing_keys` + rotation 90j | Issue JWT |
| M06 | **Magic-link channel-binding** (HMAC-JWT 30min single-use → OTP affiché en page) | 4.b.4 | auth-ms + frontend | KAYA `auth:onboard:{sessionId}` TTL 5min | Signup invitation OU recovery self |
| M07 | **OTP 8 chiffres email** | 4.b core | auth-ms + notifier-ms | KAYA `auth:otp:{otpId}` TTL 5min | OTP issue (login, grant, break-glass, step-up) |
| M08 | **OTP rate-limit** (3/5min) + lock 15min | 4.b core | auth-ms | KAYA `auth:otp:rl:{userId}`, `auth:otp:lock:{userId}` | Avant chaque OTP issue |
| M09 | **PassKey WebAuthn FIDO2** | 4.b core | auth-ms (yubico) + frontend (`@simplewebauthn`) | Postgres `device_registrations.public_key_pem` | Enrollment + assertion login |
| M10 | **TOTP RFC 6238** | 4.b core | auth-ms (samstevens-totp) | Postgres `totp_enrollments` | Enrollment + verify login |
| M11 | **Recovery codes (10 single-use)** + login factor | 4.b core / 4.b.2 | auth-ms | Postgres `recovery_codes` | Generate + use at login |
| M12 | **Device-trust fingerprint** (SHA-256 UA + IP/24 + Accept-Language) | 4.b core | auth-ms + KAYA | KAYA `dev:{userId}:{fp}` TTL 30j (configurable) | Login post-MFA OK → écrit ; login → check skip MFA |
| M13 | **WebSocket push approval + number-matching** (3 phone + 1 web) | 4.b.5 | ARMAGEDDON (proxy WS) + auth-ms (Spring WS) + frontend | KAYA `auth:approval:{requestId}` TTL 30s | Login si companion device en ligne OU step-up |
| M14 | **Risk-based scoring** (3 signaux MVP) | 4.b.6 | auth-ms + GeoLite2 | Postgres `login_history` + KAYA `auth:risk:*` | Avant chaque MFA prompt |
| M15 | **Step-up auth `@RequiresStepUp`** (PassKey/Push/TOTP/OTP, JWT 5min) | 4.b.7 | auth-ms (filter) + frontend (intercepteur) | KAYA `auth:step_up:session:*` TTL 5min | Avant ops sensibles (grant, break-glass, settings critiques) |
| M16 | **Hiérarchie rôles** SUPER-ADMIN > ADMIN > MANAGER | 4.b core | auth-ms + Keto namespace `AdminRole` | Postgres `user_roles` + Keto tuples | Tout endpoint admin |
| M17 | **Capacités fines** (~31 caps × user) | 4.b.2 | auth-ms + Keto namespace `Capability` | Postgres `account_capability_grants` + Keto tuples | Authz fine sur chaque action |
| M18 | **Capability uniqueness check** (soft warn) | 4.b.2 | auth-ms + frontend | DB query | Stepper grant role step "Capacités" |
| M19 | **SUPER-ADMIN protection** (DB trigger + service guards) | 4.b.2 | Postgres trigger + auth-ms | trigger `prevent_super_admin_delete` + service `SuperAdminProtectionService` | DELETE/SUSPEND/DEMOTE sur user SA + LAST_SA on revoke |
| M20 | **Account recovery self-initiated** (magic-link → OTP → AAL1 + force re-MFA) | 4.b.2 + 4.b.4 | auth-ms + frontend | Postgres `account_recovery_requests` | User clique "j'ai perdu accès" |
| M21 | **Account recovery admin-initiated** (token 8 chiffres TTL 1h + reset MFA cible) | 4.b.2 | auth-ms + notifier-ms | Postgres `account_recovery_requests` | SA initie depuis user-detail |
| M22 | **Audit immutable** (trigger PostgreSQL + topic admin.* Redpanda) | 4.b core / 4.b.2 | auth-ms + Postgres + Redpanda | `audit_log` append-only via trigger conditionnel | Toute action admin |
| M23 | **CAS settings + cache invalidation** (version optimistic concurrency + topic admin.settings.changed) | 4.b core | auth-ms + ARMAGEDDON cache + KAYA + BFF cache | Postgres `admin_settings` + `admin_settings_history` | PUT settings/:key |

---

## Section 2 — Architecture map (vue couches)

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Browser (Angular 21)                                                     │
│  • PassKey (M09 @simplewebauthn)  • TOTP input (qrcode)                   │
│  • <faso-otp-input> 6/8 digits   • <faso-approval-modal> (M13)            │
│  • <faso-step-up-guard> (M15)    • Recovery page /auth/recovery (M20/21)  │
└──────────────────┬───────────────────────────────────────────────────────┘
                   │ HTTPS + traceparent W3C
                   ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  ARMAGEDDON gateway (Pingora Rust :8080)                                  │
│  • Routes /api/admin/*  • ext_authz Keto :4466 (M16, M17)                │
│  • Filter SecurityHeaders (HSTS, X-Frame, CSP)                           │
│  • Filter OtpRateLimit (M08) + WS proxy /ws/admin/approval (M13)         │
│  • AdminSettingsCache (M23) consumes admin.settings.changed              │
└──────────────────┬──────────────────────────────────────┬─────────────────┘
                   │                                     │ WS subprotocol
                   ▼                                     ▼ bearer.<jwt>
┌─────────────────────────────────┐    ┌──────────────────────────────────┐
│  BFF Next.js (:4800)            │    │  auth-ms WebSocket Spring (M13)  │
│  • lib/admin-auth.ts            │    │  PushApprovalService             │
│    Kratos whoami + JWKS + Keto  │    │  Number-matching state           │
│  • Zod schemas (19+)            │    └──────────────────────────────────┘
│  • Idempotency-Key forwarding   │
│  • Audit fire-and-forget        │
└──────────────────┬──────────────┘
                   │ JWT Bearer
                   ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  auth-ms Spring Boot (:8801)                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │ Filter chain: JwtAuth → StepUpAuthFilter (M15) → RiskScoring (M14)  │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
│  Services :                                                               │
│  • CryptographicHashService (M01/M02/M03)  • OtpService (M07)             │
│  • TotpService (M10)        • WebAuthnService (M09)                       │
│  • DeviceTrustService (M12)  • RecoveryCodeService (M11)                  │
│  • AdminSettingsService (M23) • BreakGlassService                         │
│  • CapabilityService (M17)    • AccountRecoveryService (M20/M21)          │
│  • SuperAdminProtectionService (M19) • RiskScoringService (M14)           │
│  • PushApprovalService (M13) • StepUpAuthService (M15)                    │
│  • AdminMfaEnrollmentService • MagicLinkService (M06)                     │
└─────┬─────────────┬──────────────┬──────────────────┬───────────────────┘
      │             │              │                  │
      ▼             ▼              ▼                  ▼
┌──────────┐  ┌──────────┐   ┌──────────┐    ┌──────────────────┐
│ PG :5432 │  │ KAYA     │   │ Vault    │    │ Redpanda :19092  │
│ users +  │  │ :6380    │   │ :8200    │    │ auth.* admin.*   │
│ V1-V16   │  │ TTL keys │   │ peppers  │    │ 11 topics +DLQ  │
│ trigger  │  │ rate-lim │   │ JWT keys │    │ Avro envelopes   │
│ M19/M22  │  │ M07-M13  │   │ M01-M06  │    │ Schema Reg :18081│
└──────────┘  └──────────┘   └──────────┘    └──────────────────┘
                                       │
                                       │ consume
                                       ▼
                              ┌────────────────────┐
                              │ notifier-ms :8803  │
                              │ 12 templates HBS   │
                              │ 7 consumers Kafka  │
                              │ → Mailpit :1025    │
                              └────────────────────┘

┌──────────────────────────────────────────────────────────────────────────┐
│  ORY Kratos :4433 (identité)                                              │
│  • Identity schema role enum: super-admin/admin/manager/operator/viewer    │
│  • Hashers: argon2 (m=64MB, t=3, p=4)  ← M01                              │
│  • Methods: password, totp, code (legacy), webauthn (M09)                 │
│  • Hooks post-login/registration → auth-ms /internal/admin/*-event        │
└──────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────┐
│  ORY Keto :4466 read / :4467 write (autorisation)                         │
│  • Namespaces: User, Role, Platform, Resource, Department,                │
│                AdminRole (M16: super_admin/admin/manager),                │
│                Capability (M17: 31 caps × user)                           │
│  • Tuples seed: SA × 31 caps + AdminRole/<level>@<userId>                 │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Section 3 — Journées utilisateur + modules déclenchés

### Journey A — Signup ADMIN par invitation (user nouveau)

```
┌─────────────┐    ┌─────────────┐  ┌──────────┐  ┌──────────────┐  ┌─────────┐
│ SUPER-ADMIN │    │ admin-UI    │  │ BFF      │  │ auth-ms      │  │ notifier│
└──────┬──────┘    └──────┬──────┘  └────┬─────┘  └──────┬───────┘  └────┬────┘
       │ "Inviter admin"  │              │               │               │
       ├─────────────────►│              │               │               │
       │                  │ POST /me/invite              │               │
       │                  ├─────────────►│               │               │
       │                  │              │ M15 step-up?  │               │
       │                  │              ├──────────────►│ check JWT     │
       │                  │              │ 401+stepup    │ (M15 filter)  │
       │                  │              │◄──────────────┤               │
       │                  │ open modal   │               │               │
       │                  │ <step-up-    │               │               │
       │                  │  guard>      │               │               │
       │                  ├──┐ M15 PassKey re-touch       │               │
       │                  │  │           │               │               │
       │                  │◄─┘ stepUp JWT 5min            │               │
       │                  │ retry POST /me/invite         │               │
       │                  ├─────────────►├──────────────►│ M07 issue OTP │
       │                  │              │               │ (M02 hash +   │
       │                  │              │               │  HMAC pepper) │
       │                  │              │               │ M22 audit     │
       │                  │              │               │ publish topic │
       │                  │              │               │ auth.invitation.sent
       │                  │              │               │  ─ ─ ─ ─ ─ ─►│
       │                  │              │               │              │ render
       │                  │              │               │              │ admin-onboard-
       │                  │              │               │              │ magic-link.hbs
       │                  │              │               │              │
       │                  │              │               │              │ SMTP ──► email cible
       │                  │              │               │              │
─────────────────────────  email reçu, click magic link  ─────────────────────
       │                                                 │              │
       │                  ┌───────┐                      │              │
       │                  │ cible │                      │              │
       │                  └───┬───┘                      │              │
       │                      │ /auth/admin-onboard?token=jwt           │
       │                      ├────────────────────────►│ M06 verify    │
       │                      │                         │ HMAC-JWT      │
       │                      │                         │ 30min single- │
       │                      │                         │ use, KAYA jti │
       │                      │                         │               │
       │                      │                         │ M02 generate  │
       │                      │                         │ OTP 8 digits  │
       │                      │                         │ KAYA TTL 5min │
       │                      │ {sessionId, otpDisplay} │               │
       │                      │◄────────────────────────┤               │
       │                      │ user voit OTP en page   │               │
       │                      │ saisit le MÊME OTP      │               │
       │                      │ POST verify-otp         │               │
       │                      ├────────────────────────►│ M02 verify    │
       │                      │                         │ Argon2id+HMAC │
       │                      │                         │ M22 audit     │
       │                      │                         │ ONBOARD_      │
       │                      │                         │ COMPLETED     │
       │                      │ 200 + Kratos settings   │               │
       │                      │◄────────────────────────┤               │
       │                      │                         │               │
       │ ───────────  MFA enrollment OBLIGATOIRE  ───────────────────────
       │                      │ M09 PassKey enroll      │               │
       │                      │ M10 TOTP enroll (M04 AES-256-GCM)       │
       │                      │ M11 generate 10 recovery codes (M03)    │
       │                      │                         │               │
       │                      │ → /admin dashboard      │               │
```

**Modules déclenchés** : M15 step-up SA, M07 OTP issue, M22 audit, notifier consume, **M06 magic-link verify (channel-binding)**, M02 OTP hash/verify, M09 PassKey, M10 TOTP, M04 AES, M11 recovery codes, M03 hash recovery.

**Spec Playwright** : #24 `signup-magic-link-channel-binding.spec.ts`, #20 `admin-super-admin-self-management.spec.ts`.

---

### Journey B — Login régulier ADMIN (cas trusted device)

```
       ┌────────┐     ┌──────────┐    ┌─────────┐    ┌──────────┐
       │ user   │     │ admin-UI │    │ Kratos  │    │ auth-ms  │
       └───┬────┘     └────┬─────┘    └────┬────┘    └────┬─────┘
           │ /auth/login   │               │              │
           ├──────────────►│ email+pwd     │              │
           │               ├──────────────►│ M01 verify   │
           │               │               │ Argon2id     │
           │               │               │ (lazy re-    │
           │               │               │  hash si     │
           │               │               │  bcrypt)     │
           │               │               │              │
           │               │               │ /internal/admin/login-event
           │               │               ├─────────────►│
           │               │               │              │ M14 risk score
           │               │               │              │  • M12 device fp KAYA dev:* → -30
           │               │               │              │  • GeoLite2 distance → 0
           │               │               │              │  • bruteforce check → 0
           │               │               │              │ TOTAL: -30 → ALLOW
           │               │               │              │ INSERT login_history
           │               │               │              │ publish auth.risk.assessed
           │               │               │              │              
           │               │               │              │ M12 check device-trust
           │               │               │              │ KAYA dev:{u}:{fp} HIT
           │               │               │              │ → SKIP MFA
           │               │               │              │
           │               │               │              │ JWT issued + M22 audit
           │ session cookie+ JWT           │              │
           │◄──────────────────────────────┤              │
           │ /admin dashboard              │              │
```

**Modules** : M01 password Argon2id (+ lazy re-hash), M14 risk scoring (3 signaux), M12 device-trust skip MFA, M22 audit.

**Spec** : #30 `risk-scoring-known-device-low.spec.ts`.

---

### Journey C — Login depuis nouveau device (MFA + push)

```
       ┌────────┐     ┌──────────┐    ┌──────────┐    ┌─────────────┐
       │ user   │     │ admin-UI │    │ auth-ms  │    │ admin-UI #2 │
       │ device │     │ #1       │    │          │    │ (companion) │
       │  N    │     │          │    │          │    │ WS open     │
       └───┬────┘     └────┬─────┘    └────┬─────┘    └──────┬──────┘
           │ login email+pwd               │                 │
           ├───────────────►│              │                 │
           │                │ Kratos verify│                 │
           │                │              │ M01 OK          │
           │                │              │                 │
           │                │              │ M14 risk score  │
           │                │              │  • M12 device fp KAYA dev:* MISS → 0
           │                │              │  • GeoLite2 country diff → +20
           │                │              │  • bruteforce → 0
           │                │              │ TOTAL: 20 → ALLOW with MFA mandatory
           │                │              │                 │
           │                │              │ Decision: PUSH first (M13)
           │                │              │                 │
           │                │              │ check user has WS active → YES
           │                │              │ M13 generate {requestId,
           │                │              │   displayedNumber=07,
           │                │              │   phoneNumbers=[03,07,21]}
           │                │              │ KAYA auth:approval:{rid} TTL 30s
           │                │              │ publish auth.push.requested
           │                │              │                 │
           │                │  display "07"│ WS push request │
           │                │              ├────────────────►│ modal: "Login from
           │                │              │                 │ Ouagadougou? [03] [07] [21]"
           │                │              │                 │ user taps "07"
           │                │              │ WS respond      │
           │                │              │◄────────────────┤
           │                │              │ M13 verify match│
           │                │              │ KAYA status=GRANTED
           │                │              │ publish auth.push.granted
           │                │              │ M22 audit       │
           │                │              │                 │
           │                │              │ M12 write KAYA  │
           │                │              │ dev:{u}:{fp}    │
           │                │              │ TTL 30j → device│
           │                │              │ now trusted     │
           │                │              │                 │
           │ JWT issued                    │                 │
           │◄──────────────────────────────┤                 │
```

**Modules** : M01, M14 (signal +20 country diff), M13 push approval avec number-matching, M22 audit, M12 device-trust write.

**Spec** : #27 `push-approval-via-websocket.spec.ts`, #28 `push-approval-number-mismatch.spec.ts`.

---

### Journey D — Login avec recovery code (perte device)

```
       ┌────────┐     ┌──────────┐    ┌──────────┐
       │ user   │     │ admin-UI │    │ auth-ms  │
       └───┬────┘     └────┬─────┘    └────┬─────┘
           │ login email+pwd               │
           ├───────────────►│              │
           │                │ M01 OK       │
           │                │              │
           │                │ MFA prompt: PassKey/TOTP/[Code récup]/OTP
           │ choisit "Code de récupération"│
           ├───────────────►│              │
           │ saisit "7K2M-9XQF"            │
           ├───────────────►│              │
           │                │ POST /auth/login/recovery-code
           │                ├─────────────►│ M11 RecoveryCodeService.use()
           │                │              │ M03 verify Argon2id+HMAC pepper
           │                │              │ MATCH → UPDATE used_at=now
           │                │              │ M22 audit RECOVERY_CODE_USED
           │                │              │ publish auth.recovery.used
           │                │              │ remaining = 8/10
           │                │              │
           │                │              │ Si remaining=0 → email user warn
           │                │              │ + bandière persistante /admin
           │                │              │
           │                │ JWT AAL2 OK  │
           │ session         │              │
           │◄────────────────┤              │
           │
           │ Tentative 2 du MEME code:
           ├───────────────►│              │
           │                ├─────────────►│ M11 used_at != NULL → 403
           │                │              │ M22 audit RECOVERY_CODE_INVALID
           │                │ 403          │
           │◄───────────────┤              │
```

**Modules** : M01, M11 recovery use, M03 hash verify, M22 audit, KAYA `auth:recovery:lock:{userId}` après 10 fails.

**Spec critique** : #16 `admin-recovery-code-actually-works-at-login.spec.ts` — assertion **DOUBLE** : (1) le code marche au 1er essai → dashboard atteint, (2) le MÊME code rejeté au 2ème essai → 403.

---

### Journey E — Self-recovery (perte totale d'accès)

```
   ┌──────┐     ┌────────────┐    ┌─────────┐    ┌──────────┐
   │ user │     │ /auth/     │    │ BFF     │    │ auth-ms  │
   │      │     │ recovery   │    │         │    │          │
   └───┬──┘     └─────┬──────┘    └────┬────┘    └────┬─────┘
       │ "j'ai perdu accès"            │              │
       ├──────────────►│               │              │
       │               │ form email    │              │
       │               │ POST /auth/recovery/initiate │
       │               ├──────────────►│              │
       │               │               │ M14 rate-limit IP 3/h ← BFF in-mem
       │               │               ├─────────────►│ M20 AccountRecoveryService.initiateSelfRecovery
       │               │               │              │ generate HMAC-JWT 30min single-use
       │               │               │              │ INSERT account_recovery_requests
       │               │               │              │ publish auth.recovery.self_initiated
       │               │               │              │ ─ ─ ─ ─ ─ ─►notifier
       │               │               │              │             render admin-recovery-self-link.hbs
       │               │               │              │             SMTP → email user
       │               │ 200 "email envoyé"            │
       │               │◄──────────────┤              │
       │
       │ ── email reçu, user clique magic-link ──
       │
       │   /auth/recovery?token=jwt
       │               │ POST verify-link              │
       │               ├──────────────►├─────────────►│ M06 verify HMAC-JWT
       │               │               │              │ KAYA jti single-use
       │               │               │              │ M02 generate OTP 8 digits
       │               │               │              │ KAYA auth:recovery:{sid} TTL 5min
       │               │ {sessionId, otpDisplay}      │
       │               │◄─────────────────────────────┤
       │ user voit OTP en page (channel-binding)
       │ saisit OTP    │              │              │
       │               │ POST verify-otp/complete     │
       │               ├──────────────►├─────────────►│ M02 verify
       │               │               │              │ M20 mark token USED
       │               │               │              │ SET user.must_reenroll_mfa=true
       │               │               │              │ M22 audit ACCOUNT_RECOVERY_SELF_INITIATED
       │               │               │              │ Kratos AAL1 (degraded)
       │               │ 200 + redirect /admin/me/security?force-reenroll=true
       │               │◄─────────────────────────────┤
       │ bandière persistante "Réenrôlez MFA"
       │ → re-enroll PassKey + TOTP + recovery codes (M09/M10/M11)
       │ → audit ACCOUNT_RECOVERY_COMPLETED
```

**Modules** : M20 self-recovery, M06 magic-link, M02 OTP, M22 audit, M09/M10/M11 re-enrollment forcé.

**Spec** : #17 `admin-self-recovery-flow.spec.ts`.

---

### Journey F — Admin-initiated recovery (par SUPER-ADMIN)

```
   ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌────────┐
   │ SUPER-   │    │ admin-UI │    │ auth-ms  │    │ user   │
   │ ADMIN    │    │          │    │          │    │ cible  │
   └─────┬────┘    └────┬─────┘    └────┬─────┘    └────┬───┘
         │ user-detail page             │              │
         │ "Lancer récupération"        │              │
         ├───────────►│                  │              │
         │            │ modal motif≥50+OTP confirmation │
         │            │                  │              │
         │            │ M15 step-up      │              │
         │            ├─────────────────►│ check JWT step-up valid (≤5min)
         │            │                  │ → 401 require step-up
         │            │ user re-authent (PassKey re-touch)
         │            │ M15 verifyStepUp → JWT 5min
         │            │                  │              │
         │            │ POST /users/:id/recovery/initiate│
         │            ├─────────────────►│ M19 SuperAdminProtectionService
         │            │                  │ (cible peut être SA, ok ; mais pas last)
         │            │                  │ M21 AccountRecoveryService.initiateAdminRecovery
         │            │                  │ DELETE TotpEnrollments
         │            │                  │ DELETE DeviceRegistrations (M12 tuples KAYA aussi)
         │            │                  │ DELETE RecoveryCodes
         │            │                  │ generate token 8 chiffres TTL 1h
         │            │                  │ INSERT account_recovery_requests
         │            │                  │ publish auth.recovery.admin_initiated
         │            │                  │  ─ ─ ─ ─ ─►notifier
         │            │                  │           render admin-recovery-admin-token.hbs
         │            │                  │           SMTP → email cible
         │            │ 200              │              │
         │ "demande envoyée"             │              │
         │            │                  │              │
         │ ─── email reçu cible ───      │              │
         │                                              │
         │            ┌────────────┐                    │
         │            │ /auth/     │                    │
         │            │ recovery   │                    │
         │            └─────┬──────┘                    │
         │                  │                           │
         │                  │ ?adminToken=12345678 + email     │
         │                  │ POST recovery/complete           │
         │                  ├──────────────────────────────►│ M21 verify token DB
         │                  │                              │ mark used_at, status=USED
         │                  │                              │ SET user.must_reenroll_mfa=true
         │                  │                              │ M22 audit ACCOUNT_RECOVERY_COMPLETED
         │                  │ AAL1 + redirect /admin/me/security?force-reenroll=true
         │                  │ → re-enroll PassKey/TOTP/codes
```

**Modules** : M15 step-up SA, M19 SuperAdminProtection (cible), M21 admin-recovery, M22 audit, re-enrôlement.

**Spec** : #18 `admin-admin-initiated-recovery.spec.ts`.

---

### Journey G — Grant role (opération sensible avec capabilities)

```
   ┌──────┐    ┌──────────┐         ┌──────────┐
   │ ADMIN│    │ admin-UI │         │ auth-ms  │
   └───┬──┘    └────┬─────┘         └────┬─────┘
       │ /admin/users → "Gérer rôles"     │
       ├──────────►│                       │
       │           │ Stepper 5 steps :     │
       │           │ 1. Sélection target user
       │           │ 2. Capacités multi-checkbox
       │           │    GET /admin/capabilities (M17)
       │           │    POST /admin/capabilities/check-uniqueness (M18)
       │           │    if duplicate → soft warn, "Forcer" possible
       │           │ 3. Justification ≥50 chars
       │           │ 4. OTP 8 chiffres (M07/M08)
       │           │ 5. Résumé
       │           │
       │           │ POST /users/:id/roles/grant
       │           ├──────────────────────►│ M15 @RequiresStepUp → JWT step-up valid?
       │           │                       │ → 401 step_up_required
       │           │ <step-up-guard> modal │
       │           │ M15 PassKey re-touch  │
       │           │ ou Push approval (M13)│
       │           │ ou TOTP / OTP fallback│
       │           │ stepUpToken JWT 5min  │
       │           │                       │
       │           │ retry POST avec stepUpToken
       │           ├──────────────────────►│ M16 Keto check AdminRole/super_admin@actor
       │           │                       │ M17 cap check roles:grant_admin/manager
       │           │                       │ M18 if dual-control needed (target=ADMIN)
       │           │                       │      → INSERT admin_role_grants(PENDING)
       │           │                       │      → publish admin.role.grant_pending
       │           │                       │ Sinon : INSERT(APPROVED) + Keto write tuple
       │           │                       │ M17 grantCapabilities (DB + Keto Capability tuples)
       │           │                       │ if forceDuplicate → audit CAPABILITY_SET_DUPLICATE_OVERRIDE
       │           │                       │ M22 audit ROLE_GRANTED
       │           │                       │ publish auth.role.granted
       │           │                       │  ─ ─ ─ ─ ─►notifier mail cible
       │ success modal
```

**Modules** : M15 step-up obligatoire, M16 hierarchy, M17 capabilities + grant Keto, M18 uniqueness check, M07 OTP, M22 audit.

**Spec** : #21 `admin-granular-capabilities.spec.ts`, #22 `admin-grant-warns-on-duplicate-capabilities.spec.ts`, #33 `step-up-on-grant-role.spec.ts`.

---

### Journey H — SUPER-ADMIN protection (tentative malveillante)

```
   ┌──────┐    ┌──────────┐    ┌──────────┐    ┌────────────┐
   │ ADMIN│    │ admin-UI │    │ auth-ms  │    │ Postgres   │
   └───┬──┘    └────┬─────┘    └────┬─────┘    └─────┬──────┘
       │ tentative DELETE user X (qui happens to be SA)
       ├──────────►│                 │                │
       │           │ POST /users/X/suspend  (ou DELETE)
       │           ├────────────────►│                │
       │           │                 │ M19 SuperAdminProtectionService.assertNotSuperAdmin(X)
       │           │                 │ query roles → match SA
       │           │                 │ throw 403 Forbidden
       │           │                 │ M22 audit SUPER_ADMIN_PROTECTION_TRIGGERED
       │           │ 403 + audit     │                │
       │           │◄────────────────┤                │
       │           │
       │ Cas bypass code (e.g. DELETE direct via SQL injection) :
       │           │ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─►│ DELETE FROM users WHERE id=X
       │           │                       │ trigger BEFORE DELETE prevent_super_admin_delete
       │           │                       │ RAISE EXCEPTION 'SUPER_ADMIN_PROTECTION'
       │           │                       │ DataIntegrityViolationException Java side
       │           │                       │ → 403 + audit
```

**Modules** : M19 (service guard + DB trigger), M22 audit.

**Spec** : #19 `admin-super-admin-undeletable.spec.ts`.

---

### Journey I — Block high-risk login (Tor + new country + bruteforce)

```
       ┌────────────┐     ┌──────────┐    ┌──────────┐
       │ attacker   │     │ admin-UI │    │ auth-ms  │
       │ via Tor    │     │          │    │          │
       └─────┬──────┘     └────┬─────┘    └────┬─────┘
             │ login email+pwd     │              │
             ├────────────────────►│              │
             │                     │ M01 verify pwd OK
             │                     │              │
             │                     │ M14 risk score
             │                     │  • M12 device fp MISS → 0
             │                     │  • GeoLite2: France country diff → +20
             │                     │  • IP in Tor exit list → +40
             │                     │  • bruteforce 3 fails 15min → +30
             │                     │ TOTAL: 90 → BLOCK
             │                     │ INSERT login_history(decision=BLOCK)
             │                     │ publish auth.risk.blocked
             │                     │ M22 audit LOGIN_BLOCKED_HIGH_RISK
             │                     │ → notifier email user "tentative inhabituelle bloquée"
             │ 403 + traceId       │              │
             │◄────────────────────┤              │
```

**Modules** : M01, M14 (signal Tor +40 + country +20 + bruteforce +30), M22 audit, notifier alerte user.

**Spec** : #32 `risk-scoring-tor-blocked.spec.ts`.

---

## Section 4 — Cross-cutting concerns (toujours actifs)

| Concern | Module | Trigger |
|---|---|---|
| **Audit trail immutable** | M22 | Toute action admin write → INSERT audit_log + publish topic admin.* + trigger PG si immutable_mode=true |
| **Idempotency** | header `Idempotency-Key` | POST critiques (grant, break-glass, settings update, recovery generate) — KAYA cache 24h |
| **Optimistic concurrency (CAS)** | M23 | PUT /admin/settings/:key requires `version` match → 409 si stale |
| **Cache invalidation** | M23 + Redpanda | Topic `admin.settings.changed` → ARMAGEDDON cache + BFF cache 30s + KAYA cache invalidate |
| **Rate-limiting** | M08 + ARMAGEDDON OtpRateLimit | OTP 3/5min/user, login 100/min/user, recovery 3/h/IP |
| **OTel tracing** | `traceparent` W3C | Browser → BFF → ARMAGEDDON → auth-ms → KAYA/PG → Redpanda. Chaque audit_log entry porte le traceId |
| **JWT validation** | M05 | JWKS auth-ms /.well-known/jwks.json cache 10min côté BFF/ARMAGEDDON |
| **Vault peppers rotation** | M01-M03 | Champ `pepper_version` permet coexistence v1, v2, ... ; lazy re-hash on use |

---

## Section 5 — Mapping Playwright (33 specs ↔ modules)

| # | Spec | Modules clés vérifiés | Assertion critique |
|--:|---|---|---|
| 1 | `admin-signup-super-admin.spec.ts` | M07, M22 | OTP 8 digits Mailpit reçu, signup complet |
| 2 | `admin-signup-admin.spec.ts` | M06, M07, M09, M10, M11 | Magic-link → channel-binding → MFA forcé |
| 3 | `admin-signup-manager.spec.ts` | idem #2 | scope MANAGER (capabilities + level) |
| 4 | `admin-login-otp-mail.spec.ts` | M01, M02, M07, M22 | OTP regex `\b\d{8}\b` Mailpit |
| 5 | `admin-login-passkey.spec.ts` | M01, M09 | virtual authenticator CDP signs assertion |
| 6 | `admin-login-totp.spec.ts` | M01, M04, M10 | code 6 digits otplib match |
| 7 | `admin-login-recovery-code.spec.ts` | M01, M03, M11 | code XXXX-XXXX validé |
| 8 | `admin-device-trust-skip-otp.spec.ts` | M12 | 2nd login → MFA prompt **NOT visible** |
| 9 | `admin-grant-role.spec.ts` | M16, M17, M22 | Keto tuple écrit |
| 10 | `admin-revoke-role.spec.ts` | M16, M19, M22 | revoke OK sauf last SA |
| 11 | `admin-audit-query.spec.ts` | M22 | filtres date/actor/action fonctionnent |
| 12 | `admin-session-force-logout.spec.ts` | M22 + topic auth.session.revoked | session removed |
| 13 | `admin-break-glass.spec.ts` | M07, M15, M22 | TTL 4h + auto-revoke |
| 14 | `admin-settings-update.spec.ts` | M23 | CAS version + history |
| 15 | `admin-settings-effect-runtime.spec.ts` | M23 + topic admin.settings.changed | OTP length change → effect immédiat |
| 16 | `admin-recovery-code-actually-works-at-login.spec.ts` | M11, M22 | **2ème usage → 403** ⭐ |
| 17 | `admin-self-recovery-flow.spec.ts` | M20, M06, M02, M22 | magic-link → OTP → AAL1 → re-enroll |
| 18 | `admin-admin-initiated-recovery.spec.ts` | M19, M21, M22 | token 8 digits → re-enroll forcé |
| 19 | `admin-super-admin-undeletable.spec.ts` | M19 (service + trigger) | 403 + audit |
| 20 | `admin-super-admin-self-management.spec.ts` | M01, M09, M10, M11 self-mgmt | password change + new PassKey + relogin |
| 21 | `admin-granular-capabilities.spec.ts` | M17 | A peut suspend X, pas Y |
| 22 | `admin-grant-warns-on-duplicate-capabilities.spec.ts` | M18 | warn soft + force override audit |
| 23 | `crypto-argon2-rehash-on-login.spec.ts` | M01 | bcrypt → argon2id silencieux |
| 24 | `signup-magic-link-channel-binding.spec.ts` | M06 ⭐ | OTP affiché en page = saisie sur la même page |
| 25 | `signup-magic-link-tampered-token.spec.ts` | M06 | signature altérée → 401 |
| 26 | `signup-magic-link-replayed.spec.ts` | M06 KAYA jti | 2ème click → 410 |
| 27 | `push-approval-via-websocket.spec.ts` | M13 ⭐ | onglet 2 reçoit, tap "07", onglet 1 logged in |
| 28 | `push-approval-number-mismatch.spec.ts` | M13 | tap mauvais number → audit + retry |
| 29 | `push-approval-timeout.spec.ts` | M13 | 30s sans réponse → fallback OTP auto |
| 30 | `risk-scoring-known-device-low.spec.ts` | M14 | score < 30 → ALLOW direct |
| 31 | `risk-scoring-new-country-medium.spec.ts` | M14 | score 30-60 → STEP_UP forcé |
| 32 | `risk-scoring-tor-blocked.spec.ts` | M14 | score > 80 → BLOCK + email |
| 33 | `step-up-on-grant-role.spec.ts` | M15 | grant > 5min → modal step-up |

---

## Section 6 — Synthèse "à quel moment" (timeline canonique)

```
Signup ADMIN par invitation :
  T0 : SA invite (M15 step-up + M07 OTP SA)
  T1 : email envoyé (notifier consume)
  T2 : cible click magic-link (M06)
  T3 : OTP affiché en page (M02 channel-binding)
  T4 : MFA enrollment forcé (M09 + M10 + M11 + M03 + M04)
  T5 : audit (M22)

Login ADMIN :
  T0 : password (M01 + lazy rehash)
  T1 : risk scoring (M14 = M12 + GeoIP + bruteforce + Tor)
  T2 : décision ALLOW/STEP_UP/BLOCK
  T3 : si MFA requis → choice (PassKey / Push WebSocket / TOTP / OTP / recovery)
        ├─ M09 PassKey FIDO2
        ├─ M13 Push avec number-matching (companion device)
        ├─ M10 TOTP
        ├─ M07 OTP email
        └─ M11 Recovery code
  T4 : M12 device-trust write (post-MFA OK)
  T5 : audit (M22)

Op sensible (grant / break-glass / settings critique) :
  T0 : M15 @RequiresStepUp filter
  T1 : si JWT step-up > 5min → modal (M15 verify : PassKey / Push / TOTP / OTP)
  T2 : M16 Keto hierarchy + M17 capabilities check
  T3 : M19 SUPER-ADMIN protection si applicable
  T4 : action persistée + M22 audit immutable
  T5 : publish topic Redpanda → notifier consume

Recovery code login (perte device) :
  T0 : M01 password OK
  T1 : choix "code de récupération"
  T2 : saisie XXXX-XXXX
  T3 : M11 verify (M03 hash) → mark used_at + M22 audit
  T4 : si remaining=0 → email user warn

Self-recovery (perte totale) :
  T0 : POST /auth/recovery/initiate (rate-limit IP 3/h)
  T1 : M20 generate HMAC-JWT 30min
  T2 : email magic-link (notifier)
  T3 : click → M06 verify → OTP affiché en page
  T4 : saisie OTP → AAL1 + must_reenroll_mfa=true
  T5 : force re-enroll MFA + M22 audit
```

---

## TL;DR

- **23 modules de sécurité** orchestrés en couches (browser → ARMAGEDDON → BFF → auth-ms → KAYA/PG/Vault/Redpanda → notifier).
- **9 user journeys** documentés avec sequence diagrams + modules déclenchés à chaque étape.
- **33 specs Playwright** mappés 1-pour-1 sur les modules à valider.
- **Timeline canonique** pour 5 flows clés (signup, login, ops sensibles, recovery code login, self-recovery).
- Chaque audit log porte un `traceId` Jaeger pour debug E2E.

*Document de référence pour Phase 4.c (cycle-fix : check que tous les modules démarrent) et Phase 4.d (E2E : check que tous les modules fonctionnent ensemble).*
