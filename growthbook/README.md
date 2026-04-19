<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 FASO DIGITALISATION -->

# GrowthBook — Feature flags FASO DIGITALISATION

GrowthBook est la plateforme open-source de feature-flags / A/B testing
retenue par FASO DIGITALISATION. Elle s'exécute en local via `podman-compose`
avec **MongoDB** pour le stockage métadonnées et **KAYA** comme cache
haute-performance (TTL 30 s) côté SDK.

- **Port GrowthBook UI/API** : `http://localhost:3100`
- **Image** : `growthbook/growthbook:latest`
- **Backend cache SDK** : KAYA (port 6380, RESP3) — jamais Redis
- **Secrets** : Vault (`faso/growthbook/*`) — jamais en clair

## 1. Démarrage local

```bash
cd INFRA/docker/compose

# 1. Récupère les secrets depuis Vault et les exporte en env
export VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)
export GROWTHBOOK_JWT_SECRET=$(vault kv get -field=jwt      faso/growthbook/core)
export GROWTHBOOK_ENCRYPTION_KEY=$(vault kv get -field=enc  faso/growthbook/core)

# 2. Démarrer (network faso-net doit déjà exister via podman-compose.yml)
podman-compose \
  -f podman-compose.yml \
  -f ../../growthbook/podman-compose.growthbook.yml \
  up -d faso-growthbook-mongo faso-growthbook

# 3. Healthcheck
curl -fsS http://localhost:3100/healthcheck
# => {"healthy":true}

# 4. Bootstrap admin (première fois uniquement)
xdg-open http://localhost:3100
```

## 2. Créer une API key dev

Une fois connecté à `http://localhost:3100` :

1. **Settings → API Keys → Create Key**
2. Type : `SDK Connection`, Environnement : `dev`
3. Copier la clef et la placer dans Vault :

   ```bash
   vault kv put faso/growthbook/sdk \
     api_key=sdk-abcdef01234567890 \
     env=dev
   ```

4. Les backends Spring Boot la lisent via `spring-cloud-vault` (jamais en clair
   dans `application.yml`).

## 3. Seed flags de démo

```bash
GB_API_KEY=$(vault kv get -field=api_key faso/growthbook/sdk)
BASE=http://localhost:3100/api/v1

for flag in \
  '{"id":"poulets.new-checkout","description":"Nouveau tunnel de paiement (v2)","defaultValue":false,"valueType":"boolean","environments":{"dev":{"enabled":true,"rules":[{"type":"rollout","coverage":0.2}]}}}' \
  '{"id":"etat-civil.pdf-v2","description":"Actes Etat-Civil mis en page v2","defaultValue":false,"valueType":"boolean","environments":{"dev":{"enabled":true}}}' \
  '{"id":"auth.webauthn-beta","description":"WebAuthn/Passkey en bêta","defaultValue":false,"valueType":"boolean","environments":{"dev":{"enabled":true,"rules":[{"type":"rollout","coverage":0.1}]}}}' ; do
  curl -fsS -X POST "$BASE/feature" \
    -H "Authorization: Bearer $GB_API_KEY" \
    -H "Content-Type: application/json" \
    -d "$flag"
done
```

## 4. Architecture de cache

```
┌─────────────┐  GET /api/features/{env}   ┌────────────┐
│ GrowthBook  │◄───────────────────────────│  Backend   │
│  port 3100  │                            │   Java     │
└─────────────┘                            └─────┬──────┘
                                                 │ SET EX 30
                                                 ▼
                                           ┌────────────┐
                                           │    KAYA    │
                                           │ ff:env:sha │
                                           └────────────┘
                                                 │
                                  ARMAGEDDON ────┤   injecte X-Faso-Features
                                  forge middleware   ▼
                                           (upstream reçoit le header)
```

TTL 30 s : compromis **fraîcheur perçue** ≤ 30 s (UX acceptable pour
un flag non-critique) **×** **hit ratio KAYA ≥ 95 %** (à 100 req/s par
instance, 1 MISS / 30 s = 99.97 % hit).

## 5. Fichiers associés

| Chemin | Rôle |
|--------|------|
| `INFRA/growthbook/podman-compose.growthbook.yml` | Stack GrowthBook + Mongo |
| `INFRA/growthbook/FEATURE-FLAGS-README.md` | Gouvernance / lifecycle |
| `INFRA/docs/feature-flags.md` | Architecture + quand créer un flag |
| `poulets-platform/backend/src/main/java/bf/gov/faso/poulets/flags/` | SDK Java |
| `poulets-platform/frontend/src/app/core/flags/` | SDK Angular |
| `armageddon/armageddon-forge/src/feature_flag_filter.rs` | Middleware gateway |

## 6. Troubleshooting

| Symptôme | Cause | Fix |
|----------|-------|-----|
| `JWT_SECRET manquant` au boot | Vault non démarré | `podman-compose -f INFRA/vault/podman-compose.vault.yml up -d` |
| Health `/healthcheck` retourne 503 | Mongo pas healthy | `podman logs faso-growthbook-mongo` |
| SDK renvoie 401 | API key invalide ou env mismatch | Recréer clef en env `dev` |
| Backend Java : `Connection refused` KAYA | KAYA pas démarré | `podman ps \| grep kaya` ; stack principale |
