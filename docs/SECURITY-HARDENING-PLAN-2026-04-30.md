<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Plan de durcissement sécurité admin-UI v2 — Argon2id + magic-link + push approval -->

# /ultraplan — Durcissement sécurité auth FASO DIGITALISATION

**Date** : 2026-04-30
**Statut** : proposition à valider (pas d'implémentation avant gate)
**Scope** : signup + login + récupération + tous flows touchant à des secrets utilisateur

---

## Section 1 — Directive Argon2id (verrouillée)

### Décision
**Tous les hashs serveur** utilisent désormais **Argon2id** (RFC 9106). Pas de bcrypt, pas de PBKDF2, pas de scrypt nouveau code.

### Paramètres recommandés (alignés OWASP 2024)

| Usage | Memory cost (KiB) | Time cost (iterations) | Parallelism | Salt | Output |
|---|---:|---:|---:|---|---|
| **Mot de passe utilisateur** | 65 536 (64 MiB) | 3 | 4 | 16 octets random | 32 octets |
| **Code de récupération (XXXX-XXXX)** | 16 384 (16 MiB) | 2 | 2 | 16 octets | 32 octets |
| **OTP 8 chiffres** | 19 456 (19 MiB) | 2 | 1 | 16 octets | 32 octets |
| **Token magic-link** | — (pas hashé en DB ; HMAC-signed JWT) | — | — | — | — |

### Pattern HMAC + Argon2id pour OTP & codes (peppered hashing)

```
final_hash = Argon2id(
  password = HMAC-SHA256(pepper_from_vault, otp_or_code),
  salt = random_16_bytes,
  m = 19456 KiB, t = 2, p = 1,
  output_len = 32 bytes
)
```

- **Pepper** : 32 octets random, stocké **dans Vault** (path `faso/auth-ms/otp-pepper-v1` — déjà prévu en gap §13). Jamais en DB ni en config.
- **Sécurité** : si DB volée mais Vault sain → OTP 8 chiffres reste incassable (HMAC est keyed → chaque tentative en clair nécessite la pepper, inconnue de l'attaquant).
- **Rotation** : `otp-pepper-v2`, `v3` ; champ `pepper_version` sur chaque ligne `recovery_codes` / `admin_otp_requests`.

### Bibliothèques

- **Java** : `de.mkammerer:argon2-jvm:2.11` (binding JNI libargon2 — performant, maintenu) OR `org.springframework.security:spring-security-crypto` `Argon2PasswordEncoder` (pure Java, plus lent ~3x mais sans JNI).
  - **Recommandation : `argon2-jvm`** (perf 5-15ms vs 50-150ms en pure Java pour params choisis).
- **Kratos** : configurer `hashers.algorithm: argon2` dans `kratos.yml` (Kratos supporte Argon2id natif).

### Migration cryptographique
**V13** (à créer) :
```sql
-- Ajout colonnes pepper_version + algo sur tables sensibles
ALTER TABLE recovery_codes ADD COLUMN pepper_version SMALLINT DEFAULT 1;
ALTER TABLE recovery_codes ADD COLUMN hash_algo VARCHAR(16) DEFAULT 'argon2id';
ALTER TABLE admin_otp_requests ADD COLUMN pepper_version SMALLINT DEFAULT 1;
ALTER TABLE admin_otp_requests ADD COLUMN hash_algo VARCHAR(16) DEFAULT 'argon2id';
-- Idem pour user.password_hash si l'auth-ms gère lui-même le hash
ALTER TABLE users ADD COLUMN hash_algo VARCHAR(16) DEFAULT 'argon2id';
ALTER TABLE users ADD COLUMN hash_params JSONB; -- {m: 65536, t: 3, p: 4, version: 19}
```

### Migration des hashs existants (re-hash on login)
Stratégie **lazy** : à chaque login réussi, si `hash_algo != 'argon2id'` ou `pepper_version != current`, re-hasher silencieusement et persister. Aucune migration batch (qui nécessiterait le mot de passe en clair).

**Pour OTP/recovery codes existants** : pas de migration possible (codes one-shot ou TTL court) → pure forward.

### Service unifié `CryptographicHashService`
```java
public interface CryptographicHashService {
  String hashPassword(char[] plaintext);
  boolean verifyPassword(char[] plaintext, String storedHash, HashParams params);
  String hashOtp(String code);                    // HMAC + Argon2id with pepper
  boolean verifyOtp(String code, String storedHash, int pepperVersion);
  String hashRecoveryCode(String code);
  boolean verifyRecoveryCode(String code, String storedHash, int pepperVersion);
}
```

---

## Section 2 — Mon avis sur ta proposition (magic-link → OTP hybride)

### Reformulation
> User saisit son email → serveur envoie email avec lien magique → user clique → serveur certifie l'ownership de l'email + génère un OTP 8 chiffres affiché à l'étape suivante du workflow → user saisit l'OTP sur l'onglet web original → connexion.

### Analyse

✅ **Forces** :
1. **Channel-binding** : la propriété "email" est prouvée par le clic sur le lien (control flow côté serveur).
2. **Device-binding** : l'OTP saisi sur l'onglet web original prouve la continuité de session sur le device de départ. Si l'attaquant a juste compromis l'email, il ne peut pas voir l'OTP affiché sur ton browser.
3. **Anti-replay** : magic link single-use + OTP TTL 5min + lock après 3 fails.
4. **Anti-phishing** : si le lien est `https://admin.faso.bf/auth/verify?token=...` et que le user vérifie l'URL avant de cliquer.

⚠️ **Faiblesses** :
1. **Si tu n'utilises QUE ce flow** (pas de password + MFA) → un attaquant qui compromet l'email **et** voit l'écran web (malware) gagne. Donc ce pattern doit rester un facteur ADDITIONNEL, pas remplacer le password+MFA.
2. **UX lourde** au signup (1 email + 1 attente OTP) — acceptable pour un compte ADMIN sensible, lourd pour login quotidien.
3. **Anti-MFA-bombing** non couvert : si l'attaquant a le password, il peut spammer le flow et espérer que tu cliques par fatigue.

### Verdict
**Excellent au signup et lors d'opérations sensibles** (octroi de droit, break-glass, recovery). **Trop lourd au login quotidien** où PassKey + risk-based step-up sont meilleurs.

### Mon contre-proposition affinée
- **Au signup ADMIN** : magic-link → OTP 8 chiffres (channel-binding) → enrôlement obligatoire PassKey + TOTP. C'est ce que tu décris : adopté.
- **Au login régulier** : PassKey si enrôlée (FIDO2 phishing-resistant) + risk-based step-up (cf. §4 Tier 5).
- **Pour opérations sensibles** (grant role, break-glass, settings critiques) : **Re-authentification step-up** = re-enter password + push approval ou TOTP.
- **Pour récupération de compte perdu** : magic-link → OTP 8 chiffres → re-enrollment MFA forcé. C'est aligné avec le delta §5.

---

## Section 3 — Comparatif des patterns "approbation depuis un autre appareil"

| Pattern | Description | Phishing-resistant | UX | Sovereignty |
|---|---|---|---|---|
| **A. Magic-link + OTP hybride (ta proposition)** | Email → click → OTP affiché → saisie | ⚠️ moyen (OK si URL vérifiée) | Lourd | ✅ |
| **B. Push approval "OUI/NON"** (Duo, MS Authenticator) | Push notif sur mobile → tap "Approuver" | ⚠️ vulnérable au MFA bombing | Très bonne | ⚠️ FCM/APN = cloud foreign |
| **C. Number-matching push** (MS 2024) | Push montre 3 chiffres ; web montre 1 ; user tap le bon | ✅ Très bon (anti-MFA-bombing) | Bonne | ⚠️ FCM/APN |
| **D. Cross-device WebAuthn (CTAP 2.2 hybrid)** | Web montre QR → phone scan → BLE/cloud signaling → phone signe assertion FIDO2 | ✅✅ Excellent (FIDO2 = phishing-immune) | Bonne | ✅ (W3C standard) |
| **E. Code-from-2nd-device** (GitHub style) | Web montre 6 chiffres ; user tape sur phone app | ⚠️ moyen | Moyenne | Variable |
| **F. WebSocket "I'm online" approval** | Companion device (browser ou PWA) maintient WebSocket → web demande approbation → user tap dans l'autre tab | ✅ bon | Bonne | ✅ Souverain (via ARMAGEDDON) |

### Mes recommandations FASO

1. **Tier obligatoire** : **Pattern D (WebAuthn cross-device)** — déjà couvert par `@simplewebauthn` côté frontend si user enrôle une PassKey sur smartphone. Aucune dépendance externe, FIDO2 standard.
2. **Tier optionnel souverain** : **Pattern F (WebSocket)** — le user qui a 2 onglets/devices ouverts sur l'admin-UI peut valider depuis le second. Pas de FCM/APN. Implémentable via Pingora+ARMAGEDDON `/ws/admin/approval`.
3. **Pattern B/C (push FCM)** : **rejeté** par souveraineté FASO (CLAUDE.md rule §3 — pas de cloud foreign).
4. **Pattern A (ta proposition)** : adopté pour les opérations sensibles + récupération.

---

## Section 4 — Architecture cible 5-tiers de sécurité

### Tier 1 — Identity vault (déjà prévu)
- Argon2id + HMAC pepper partout (cf. §1).
- AES-256-GCM at rest pour TOTP secrets.
- JWT ES384 signé par auth-ms.
- Vault pour les peppers et clés JWT.

### Tier 2 — Signup ADMIN ultra-sécurisé (nouveau)
**Workflow** :
1. SUPER-ADMIN → `/admin/users → Inviter un admin` (déjà existant).
2. Cible reçoit email avec **magic-link signé** (HMAC-JWT, single-use, TTL 30min).
3. Cible clique → page `/auth/admin-onboard?token=...` → vérifie token + génère **OTP 8 chiffres** affiché en page (PAS envoyé par mail — channel-binding).
4. Cible entre OTP sur la même page → backend valide → flow continue.
5. Étape suivante : **enrôlement PassKey OBLIGATOIRE** (`@simplewebauthn`).
6. Étape suivante : **enrôlement TOTP** (QR + verify).
7. Étape suivante : **génération + téléchargement de 10 recovery codes**.
8. Cible peut maintenant se logger.

### Tier 3 — Login régulier (adaptive)
**Décision tree** :
```
Si device_trusted (KAYA dev:{userId}:{fp} TTL 30j) :
  → Password + PassKey (silence sur OTP/TOTP)
Sinon si PassKey enrôlée :
  → Password + PassKey
Sinon si TOTP enrôlé :
  → Password + TOTP + email OTP 8 chiffres (parce que pas de PassKey = device suspect)
Sinon (que recovery codes) :
  → Password + recovery code + force re-enrollment MFA après login
```

### Tier 4 — Step-up auth pour opérations sensibles (nouveau)
**Quand** : grant role, revoke role, suspend user, settings update (catégorie `audit`/`mfa`/`grant`/`break_glass`), break-glass activate, account recovery initiate.

**Comment** : avant validation finale, re-authentification courte :
- Option A : OTP 8 chiffres email/SMS (existant).
- Option B (NOUVEAU) : **push approval via WebSocket** (Pattern F) si companion device en ligne.
- Option C : PassKey re-touch (browser prompt rapide).

UX : modal "Confirmation requise — touchez votre PassKey" avec timeout 30s.

### Tier 5 — Risk-based scoring (nouveau)
**Service `RiskScoringService`** dans auth-ms. Calcule un score 0-100 à chaque login :

| Signal | Poids | Source |
|---|---:|---|
| Device fingerprint match (KAYA) | -30 (baisse risque) | DeviceTrustService |
| Geo IP cohérente avec dernière session | -20 | MaxMind GeoLite2 ou base IP-RIPE Africa |
| Heure de login dans plage habituelle (IQR ±2h) | -10 | KAYA `auth:login-stats:{userId}` |
| User agent identique à dernière session | -10 | KAYA |
| Tentative de login après échec récent (15min) | +30 | BruteForceService |
| Velocity (logins multiples en <5min) | +25 | KAYA window |
| IP listée dans Tor/VPN/proxy connu | +40 | Liste statique mise à jour |
| Première connexion depuis ce pays | +20 | GeoIP |

**Décisions** :
- Score < 30 : login normal (Tier 3 standard).
- Score 30-60 : step-up MFA obligatoire même si trusted device.
- Score 60-80 : step-up + email notification au user "tentative inhabituelle".
- Score > 80 : block + alerte SUPER-ADMIN + audit `LOGIN_BLOCKED_HIGH_RISK`.

Stockage stats : KAYA `auth:risk:{userId}` (sliding window 30j).

---

## Section 5 — Push approval souverain (Pattern F — WebSocket)

### Architecture
```
Browser onglet 2 (déjà loggé sur /admin)        Browser onglet 1 (login en cours)
        │                                                      │
        │ WS persistent /ws/admin/approval                    │
        │                                                      │
        └────────────► ARMAGEDDON :8080 ◄──────────────────────┘
                            │
                       auth-ms /ws/approval-relay
                            │
                       KAYA `auth:approval:{requestId}` TTL 30s
```

### Flux
1. Onglet 2 (admin déjà connecté) ouvre WebSocket `/ws/admin/approval` à l'init de la page admin.
2. Onglet 1 demande login → backend détecte qu'un WS est ouvert pour ce user.
3. Backend génère `approvalRequest = {requestId, location, ip, ua, timestamp}` + l'envoie via WS à onglet 2.
4. Onglet 2 affiche modal "Login depuis Ouagadougou (196.28.111.42, Chrome 124) ? [APPROUVER] [REFUSER]".
5. **Number-matching** : onglet 1 affiche "07", onglet 2 affiche `[03, 07, 21]` → user tap "07".
6. Onglet 2 envoie réponse signée via WS.
7. Backend valide → onglet 1 continue le login.

### Avantages
- **Souverain** : toute la stack (ARMAGEDDON Pingora + auth-ms + KAYA) est FASO.
- **Phishing-resistant** : si user n'a pas un onglet ouvert, fallback OTP normal.
- **Anti-MFA-bombing** : number-matching empêche le tap-réflexe.
- **Pas de FCM/APN** : aucun cloud foreign impliqué.

### Future PWA companion
Phase ultérieure : packaging Angular en PWA + Web Push API. Web Push routes par défaut via FCM Chrome — **non souverain**. Alternative : Mozilla autopush self-hosted (open-source) → routage via service souverain. À explorer en Phase 5+.

---

## Section 6 — Plan d'implémentation (sub-phases)

### Phase 4.b.3 — Crypto upgrade Argon2id (1 semaine)
- [ ] Migration V13 : ajout colonnes `pepper_version`, `hash_algo`, `hash_params` sur tables sensibles.
- [ ] Service `CryptographicHashService` (Java) avec `argon2-jvm`.
- [ ] Service `OtpHashService` (HMAC-SHA256 + Argon2id avec pepper Vault).
- [ ] Vault seed `faso/auth-ms/otp-pepper-v1` + `password-pepper-v1` (32 octets random openssl).
- [ ] Mise à jour `OtpService`, `RecoveryCodeService`, `AdminLoginService` pour appeler le service unifié.
- [ ] Lazy re-hash on login si algorithme legacy.
- [ ] Kratos config : `hashers: argon2: {memory: 65536, iterations: 3, parallelism: 4}`.
- [ ] Audit : action `HASH_REHASHED_ON_LOGIN` (silencieuse, métrique uniquement).

### Phase 4.b.4 — Magic-link channel-binding au signup (1 semaine)
- [ ] Endpoint `POST /admin/auth/onboard/begin` (publié par invitation email — utilise déjà topic `auth.invitation.sent` du Stream A).
- [ ] Endpoint `POST /admin/auth/onboard/verify-link?token=...` → vérifie HMAC-JWT + génère OTP affiché en page.
- [ ] Endpoint `POST /admin/auth/onboard/verify-otp` → valide OTP + force MFA enrollment.
- [ ] Page Angular publique `/auth/admin-onboard` (3-step : verify-link → OTP entry → MFA enroll redirect).
- [ ] Topic `auth.onboard.completed` Redpanda.
- [ ] Mise à jour template Handlebars `admin-invitation.hbs` : lien `https://admin.faso.bf/auth/admin-onboard?token=...` (au lieu d'un lien recovery générique).

### Phase 4.b.5 — WebSocket push approval (2 semaines)
- [ ] Endpoint WS `/ws/admin/approval` côté ARMAGEDDON (Pingora WS proxy → auth-ms).
- [ ] Service `PushApprovalService` (auth-ms) : registre des connexions WS par user, génère requests, attend réponse.
- [ ] Storage approvals en KAYA `auth:approval:{requestId}` TTL 30s.
- [ ] Frontend : composant `<faso-approval-modal>` qui s'abonne via WS, affiche number-matching, envoie réponse signée.
- [ ] Hook dans login flow : si user a session WS active, propose approbation au lieu d'OTP par défaut.
- [ ] Setting `mfa.push_approval_enabled` (bool, default true) dans Configuration Center.
- [ ] Audit `PUSH_APPROVAL_REQUESTED`, `PUSH_APPROVAL_GRANTED`, `PUSH_APPROVAL_DENIED`, `PUSH_APPROVAL_TIMEOUT`.

### Phase 4.b.6 — Risk-based scoring (1-2 semaines)
- [ ] Service `RiskScoringService` (auth-ms) avec calcul score (signaux du tableau §4 Tier 5).
- [ ] Stockage stats : KAYA `auth:risk:{userId}` (sliding window 30j) + table `login_history`.
- [ ] Lib GeoIP : MaxMind GeoLite2 self-hosted (license MIT pour usage non-commercial — vérifier conformité AGPL).
- [ ] Liste Tor/VPN : ingérée depuis `https://check.torproject.org/torbulkexitlist` quotidienne.
- [ ] Hook dans login flow : appel `riskScoringService.score(loginContext)` après password verify.
- [ ] Settings dans Configuration Center : seuils `risk.score_threshold_step_up` (default 30), `risk.score_threshold_block` (default 80).
- [ ] Audit `LOGIN_RISK_ASSESSED`, `LOGIN_BLOCKED_HIGH_RISK`, `LOGIN_STEP_UP_REQUIRED`.

### Phase 4.b.7 — Step-up auth pour opérations sensibles (1 semaine)
- [ ] Annotation Java `@RequiresStepUp(maxAgeSeconds=300)` sur méthodes sensibles.
- [ ] Filter intercepteur : si `Last-Step-Up` JWT claim > maxAge → 401 + body `{ require: 'step-up', methods: ['passkey', 'totp', 'push-approval'] }`.
- [ ] Frontend : composant `<faso-step-up-guard>` qui ouvre modal de re-authentification.
- [ ] Mise à jour endpoints : grant role, revoke role, settings update sensibles, break-glass, recovery initiate.

---

## Section 7 — Tests E2E à AJOUTER (Phase 4.c — extension)

| # | Spec | Couverture |
|---|---|---|
| 23 | `crypto-argon2-rehash-on-login.spec.ts` | User legacy bcrypt → login → assert hash en DB devient argon2id |
| 24 | `signup-magic-link-channel-binding.spec.ts` | Invite admin → email Mailpit → click link → page affiche OTP → saisie sur même page → succès |
| 25 | `signup-magic-link-tampered-token.spec.ts` | Modifier signature JWT → 401 |
| 26 | `signup-magic-link-replayed.spec.ts` | Click 2× → 2ᵉ click → 410 Gone |
| 27 | `push-approval-via-websocket.spec.ts` | 2 onglets → login onglet 1 → modal onglet 2 number-matching → tap correct → onglet 1 logged in |
| 28 | `push-approval-number-mismatch.spec.ts` | Tap mauvais nombre → audit `WRONG_NUMBER_TAPPED` + retry |
| 29 | `push-approval-timeout.spec.ts` | Pas de réponse 30s → fallback OTP automatique |
| 30 | `risk-scoring-known-device-low.spec.ts` | Device trusted + IP same → score < 30 → login direct |
| 31 | `risk-scoring-new-country-medium.spec.ts` | New IP cherchant pays différent → score 30-60 → step-up forcé |
| 32 | `risk-scoring-tor-blocked.spec.ts` | IP Tor → score > 80 → block + audit |
| 33 | `step-up-on-grant-role.spec.ts` | Grant role > 5min après login → modal re-auth → PassKey touch → succès |

---

## Section 8 — Risques & mitigations

| Risque | Sévérité | Mitigation |
|---|---|---|
| Argon2id paramètres trop élevés → DoS sur login | HAUTE | Bench préalable sur target hardware, paramètres tunables via Configuration Center `crypto.argon2.memory_kib` |
| Pepper Vault perdu → tous OTP/codes invalidés | CRITIQUE | Backup Vault automatique (déjà policy FASO), versioning pepper (v1, v2 coexistent) |
| Magic-link interceptable (proxy d'entreprise log les URLs) | MOYENNE | Token JWT court (30min) + single-use + audit LINK_REPLAY |
| WebSocket DDoS | MOYENNE | Rate-limit ARMAGEDDON `armageddon:ws:rl:{userId}` 10 connexions/min, idle timeout 5min |
| Risk scoring false positives bloquent users légitimes | HAUTE | Seuils configurables, bypass via SUPER-ADMIN, audit LOGIN_RISK_ASSESSED traçable |
| GeoLite2 license commerciale floue avec AGPL | MOYENNE | Alternative `IP2Location LITE` ou base RIPE Africa custom — à valider légalement |
| Cross-device WebAuthn nécessite phone moderne | BASSE | Fallback TOTP toujours disponible |

---

## Section 9 — Décisions à valider AVANT implémentation

1. **Argon2id params** OWASP 2024 acceptés (m=64MiB, t=3, p=4 password) ?
2. **Bibliothèque Java** : `argon2-jvm` (JNI, perf) vs `spring-security-crypto` (pure Java) ?
3. **Magic-link channel-binding au signup uniquement**, ou aussi sur récupération ? *Recommandation : signup + recovery (déjà aligné avec Phase 4.b.2 amendments).*
4. **Push approval WebSocket** : périmètre Phase 4.b.5 OU reporté à Phase 5 (post-MVP) ? *Recommandation : 4.b.5 (différenciateur fort, souverain).*
5. **Risk-based scoring** : Phase 4.b.6 OU reporté ? *Recommandation : MVP avec 3 signaux (device + IP + bruteforce), élargi en Phase 5.*
6. **Number-matching** sur push approval (anti-MFA-bombing) : oui par défaut ?
7. **GeoIP** : MaxMind GeoLite2 (license gratuite mais commerciale) ou base IP-RIPE Africa custom ? *Validation juridique nécessaire.*

---

## Section 10 — Ordre d'enchaînement avec phases en cours

1. **Aujourd'hui** : amendments Phase 4.b.2 en cours (capabilities, recovery, self-mgmt). À leur retour → consolidation.
2. **Phase 4.b.3** (Argon2id + HMAC pepper) — **prioritaire**, 1 semaine.
3. **Phase 4.b.4** (magic-link channel-binding signup) — 1 semaine.
4. **Phase 4.b.5** (push approval WebSocket) — 2 semaines.
5. **Phase 4.b.6** (risk-based scoring) — 1-2 semaines.
6. **Phase 4.b.7** (step-up auth) — 1 semaine.
7. **Phase 4.c** (E2E avec **22+11=33 specs**) — 2 semaines.
8. **Phase 4.d** (cycle-fix) — 1 semaine.

**Total estimation** : Phase 4.b complet = **6-7 semaines** ; Phase 4.c+d = **3 semaines**.

---

## TL;DR — Mes 3 recommandations clés

1. ✅ **Argon2id + HMAC pepper Vault** : standard moderne, accepté.
2. ✅ **Magic-link → OTP channel-binding au signup admin** (ta proposition) : adopté tel quel, c'est un excellent pattern pour comptes sensibles. Adopté aussi pour la récupération.
3. ✅ **Push approval via WebSocket souverain (Pattern F)** au lieu de FCM/APN : différenciateur FASO, anti-MFA-bombing avec number-matching, pas de cloud foreign. **Mon vote fort.**

---

*Plan de durcissement à valider avant implémentation Phase 4.b.3 → 4.b.7.*
