# Kratos MFA — configuration ops

Guide à appliquer côté `shared-infrastructure/ory/kratos/kratos.yml` (ou config locale dev)
pour activer les méthodes MFA exposées par le frontend Poulets BF.

## 1. Activation des méthodes

```yaml
selfservice:
  methods:
    password:
      enabled: true
    webauthn:
      enabled: true
      config:
        rp:
          id: poulets.fasodigitalisation.bf          # prod
          display_name: Poulets BF
          origins:
            - https://poulets.fasodigitalisation.bf
            - http://localhost:4801
        passwordless: false
    totp:
      enabled: true
      config:
        issuer: "Poulets BF"
    lookup_secret:
      enabled: true
    code:
      enabled: true
      config:
        lifespan: 15m
    link:
      enabled: true
      config:
        lifespan: 1h
    profile:
      enabled: true

  flows:
    settings:
      required_aal: aal2
      ui_url: http://localhost:4801/profile/mfa
      lifespan: 1h
      privileged_session_max_age: 15m
    login:
      ui_url: http://localhost:4801/auth/login
      lifespan: 1h
    registration:
      ui_url: http://localhost:4801/auth/register
      lifespan: 1h
      after:
        password:
          hooks:
            - hook: session
            - hook: show_verification_ui
    recovery:
      enabled: true
      use: code
      ui_url: http://localhost:4801/auth/forgot-password
      lifespan: 1h
    verification:
      enabled: true
      use: code
      ui_url: http://localhost:4801/auth/verify
      lifespan: 1h

session:
  lifespan: 24h
  whoami:
    required_aal: highest_available
```

## 2. Identity schema — trait `mfa_onboarding_completed`

Ajouter au schéma JSON identity :

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "properties": {
    "traits": {
      "type": "object",
      "properties": {
        "email": {
          "type": "string",
          "format": "email",
          "ory.sh/kratos": {
            "credentials": { "password": { "identifier": true }, "webauthn": { "identifier": true } },
            "verification": { "via": "email" },
            "recovery": { "via": "email" }
          }
        },
        "mfa_onboarding_completed": {
          "type": "boolean",
          "default": false
        },
        "phone":    { "type": "string", "maxLength": 20 },
        "role":     { "type": "string", "enum": ["ELEVEUR", "CLIENT", "PRODUCTEUR", "ADMIN"] }
      },
      "required": ["email", "role"]
    }
  }
}
```

## 3. Templates courier (SMTP Gmail ou Mailhog dev)

Placer sous `infrastructure/ory/kratos/templates/` :

```
verification_code/valid/email.subject.gotmpl  → "Vérifiez votre compte Poulets BF"
verification_code/valid/email.body.gotmpl     → template HTML avec code
login_code/valid/email.subject.gotmpl         → "Votre code de connexion Poulets BF"
login_code/valid/email.body.gotmpl
recovery_code/valid/email.subject.gotmpl      → "Récupération de votre compte Poulets BF"
recovery_code/valid/email.body.gotmpl
```

Config SMTP (référence PLAN_MFA_ONBOARDING.md d'ETAT-CIVIL) :

```yaml
courier:
  smtp:
    connection_uri: smtps://fasodigitalisation@gmail.com:$GMAIL_APP_PW@smtp.gmail.com:465
    from_address: fasodigitalisation@gmail.com
    from_name: Poulets BF
  template_override_path: /etc/config/kratos/templates
```

Secret Vault : `secret/smtp/gmail_app_password`.

## 4. Proxy frontend → Kratos (dev)

Poulets dev sert sur 4801 ; Kratos public sur 4433. Ajouter dans `proxy.conf.json` :

```json
{
  "/self-service/*": {
    "target": "http://localhost:4433",
    "secure": false,
    "changeOrigin": true
  },
  "/sessions/whoami": {
    "target": "http://localhost:4433",
    "secure": false,
    "changeOrigin": true
  }
}
```

Puis `ng serve --port 4801 --proxy-config proxy.conf.json` pour éviter les CORS en dev.

## 5. Tests manuels

Après activation de Kratos :

1. `podman-compose -f INFRA/docker/compose/podman-compose.yml up -d kratos mailhog auth-ms`
2. Créer un compte test via `/auth/register` → vérifier code reçu dans Mailhog (http://localhost:8025)
3. Se connecter (AAL1) → naviguer `/profile/mfa`
4. Ajouter une PassKey (prompt navigateur biométrique)
5. Configurer TOTP (scan QR avec Google Authenticator / Authy)
6. Générer 10 backup codes → sauvegarder le .txt
7. Logout → re-login avec password → être redirigé `/auth/mfa?return_to=/profile/mfa` (AAL2 required)
8. Valider avec PassKey → retour `/profile/mfa`
9. `/profile/security` → liste des sessions actives

## 6. Production

- Remplacer `origins: [http://localhost:4801]` par `[https://poulets.fasodigitalisation.bf]`
- Activer HSTS, CSRF Kratos cookies : `cookie_domain: fasodigitalisation.bf, secure: true, same_site: Lax`
- Monitoring Kratos p99 login : exposé via `/admin/monitoring` dashboard Poulets (service « ORY Kratos »)
- SMS : activer courier SMS Kratos (ou notification-service Spring) avec provider africain (Africa's Talking, Twilio, Orange SMS API)

## Références code source

- Frontend Poulets : `src/app/core/kratos/kratos-settings.service.ts`, `src/app/features/profile/components/mfa-settings.component.ts`, `src/app/features/auth/components/mfa-challenge.component.ts`
- Spec originelle : `fasodigit/Etat-civil/PLAN_MFA_ONBOARDING.md`
- Auth service Kratos : `fasodigit/Etat-civil/frontend/actor-ui/src/app/core/auth/auth.service.ts`
