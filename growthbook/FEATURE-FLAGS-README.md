# FASO DIGITALISATION — Feature flags (GrowthBook + cache KAYA)

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## Architecture

```
┌─────────────┐       ┌────────────┐        ┌────────────┐
│ GrowthBook  │◄──────│  Backend   │◄──────►│   KAYA     │
│ (port 3100) │  API  │ (SDK Java) │  cache │  SET EX 30 │
└─────────────┘       └────────────┘        └────────────┘
                           │
                           ▼
                     Header X-Faso-Flags
                           │
                     ┌─────────────────┐
                     │   ARMAGEDDON    │
                     │ FeatureFlagFilter
                     └─────────────────┘
```

## Démarrage

```bash
cd INFRA/docker/compose
echo "$(openssl rand -base64 32)" > secrets/growthbook_jwt_secret.txt
echo "$(openssl rand -base64 32)" > secrets/growthbook_encryption_key.txt
podman-compose -f docker-compose.yml -f ../../growthbook/docker-compose.growthbook.yml \
  up -d growthbook-mongo growthbook

open http://localhost:3100   # bootstrap admin account
```

## SDK Java (auth-ms / notifier-ms / poulets-backend)

```xml
<!-- pom.xml -->
<dependency>
  <groupId>io.growthbook.sdk</groupId>
  <artifactId>growthbook-sdk-java</artifactId>
  <version>0.9.100</version>
</dependency>
```

`FeatureFlagService.java` (à créer par service) :
- Fetch `features.json` depuis GrowthBook API
- Cache KAYA `SET faso:flags:<env>:<sha> <json> EX 30`
- Fallback stale (1h) si API down
- Metrics Prometheus `feature_flag_evaluations_total{flag,result}`

## SDK TypeScript (BFF / Angular)

```bash
cd INFRA/poulets-platform/bff && bun add @growthbook/growthbook@1.4.0
cd INFRA/poulets-platform/frontend && npm install @growthbook/growthbook@1.4.0
```

## ARMAGEDDON middleware

`armageddon-forge/src/feature_flags.rs` (livré) : `FeatureFlagService` thread-safe, cache ArcSwap, injection `X-Faso-Flags`.

## Gouvernance flags

| Lifecycle | Action |
|-----------|--------|
| **Create** | PR + review produit, objectif mesurable, sunset date obligatoire |
| **Active** | Tracking usage via metrics Prometheus |
| **Sunset** | 30 j avant fin : issue auto "Remove flag X" |
| **Archive** | Flag retiré du code, kept in history |

## Flags initiaux recommandés

- `enable-2fa-enforcement` (auth-ms) — force TOTP/WebAuthn citoyens
- `use-new-stock-calculation` (poulets) — algorithme v2
- `mail-html-dark-mode` (notifier) — dark theme mails
- `armageddon-response-cache-enabled` (ARMAGEDDON A9, kill-switch)
- `poulets-halal-certification-mandatory` — réglementation progressive
