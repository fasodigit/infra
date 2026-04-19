<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 FASO DIGITALISATION -->

# Feature flags FASO DIGITALISATION

Document de référence — architecture, gouvernance et choix techniques
du dispositif de feature-flags / A/B testing FASO.

## 1. Pourquoi des feature flags

- **Découplage déploiement / activation** : on pousse du code éteint, on
  allume quand produit + métier donnent feu vert.
- **Kill-switch** : désactivation instantanée d'une fonctionnalité en
  incident, sans rollback.
- **Rollout progressif** : 1 % → 10 % → 50 % → 100 % des citoyens.
- **A/B testing** : mesurer l'impact UX/conversion d'un changement.
- **Segmentation** : flag actif pour `role=admin` uniquement, pour `region=Ouagadougou`, etc.

## 2. Architecture

```
┌───────────────┐
│   GrowthBook  │  (MongoDB persistent store, port 3100)
└───────┬───────┘
        │  HTTP GET /api/features/{env}      [API key Vault]
        ▼
┌───────────────┐   SET EX 30   ┌──────────┐
│  Backend Java │──────────────►│   KAYA   │  (port 6380, RESP3)
│  Spring Boot  │◄──────────────│  cache   │
└───────┬───────┘    GET        └──────────┘
        │
        │  HTTP response header  X-Faso-Features: flag1,flag2
        ▼
┌───────────────┐
│ Angular 21 FE │  FeatureFlagsService + *fasoFeature directive
└───────────────┘

                  ┌──────────────────┐
Requêtes entrantes │    ARMAGEDDON    │  FeatureFlagFilter (tower::Layer)
───────────────────►  forge middleware │  lit X-User-Id, GET KAYA,
                  │  (Rust souverain) │  injecte X-Faso-Features upstream
                  └──────────────────┘
```

### Composants

| Couche    | Composant                              | Rôle                                      |
|-----------|----------------------------------------|-------------------------------------------|
| Source    | GrowthBook (MongoDB)                   | Définition + règles + audit des flags     |
| Cache     | **KAYA** (port 6380, RESP3, sovereign) | TTL 30 s par clef `ff:{env}:{hash}`       |
| Backend   | Spring Boot `FeatureFlagsService`      | REST GrowthBook + cache KAYA + évaluation |
| Gateway   | ARMAGEDDON `FeatureFlagFilter` (tower) | Injection header upstream                 |
| Frontend  | Angular `FeatureFlagsService`          | Bootstrap via `APP_INITIALIZER`, `flags$` |
| Directive | `*fasoFeature="'key'"`                 | Rendu conditionnel du template            |

### Clef de cache KAYA

`ff:{env}:{hash16}` où `hash16 = SHA-256(canonical_json(attributes))[..8]` (16 hex chars).

- `env` ∈ `{dev, staging, prod}` (mapping GrowthBook)
- Même algorithme côté Java (`FeatureFlagsService.attributesHash`) et Rust
  (`FeatureFlagFilter::cache_key`) pour **partage transparent** du cache.

## 3. Quand créer un flag

| Cas d'usage                                                  | Flag ?    |
|--------------------------------------------------------------|-----------|
| Refonte UX incertaine (A/B)                                  | **Oui**   |
| Changement de règle métier (ex. calcul TVA)                  | **Oui**   |
| Déploiement progressif par région / rôle                     | **Oui**   |
| Kill-switch d'une intégration tierce (Orange Money, SMS…)    | **Oui**   |
| Correction de bug simple, peu risquée                        | Non       |
| Migration irréversible (schema DB)                           | Non       |
| Feature permanente (core business)                           | Non       |

**Règles de hygiène** :

- Un flag doit avoir une **sunset date** à la création (≤ 90 j par défaut).
- Un flag **archivé** est retiré du code *avant* d'être retiré de GrowthBook.
- L'usage d'un flag est mesuré via métrique Prometheus
  `feature_flag_evaluations_total{flag,result}` — dead flags détectés à 0 eval/j.
- Les clefs suivent la convention `domaine.nom-en-kebab` : `poulets.new-checkout`,
  `etat-civil.pdf-v2`, `auth.webauthn-beta`.

## 4. TTL 30 s — justification

Le choix d'un TTL de **30 secondes** sur le cache KAYA est un compromis
entre **fraîcheur perçue** et **charge sur GrowthBook + KAYA**.

| TTL     | Fraîcheur max | Hit ratio (100 RPS/inst) | Charge GrowthBook |
|---------|---------------|--------------------------|-------------------|
| 0 (off) | instantané    | 0 %                      | 100 RPS           |
| 5 s     | 5 s           | 99.80 %                  | 0.2 RPS           |
| **30 s**| **30 s**      | **99.97 %**              | **0.033 RPS**     |
| 5 min   | 5 min         | 99.997 %                 | 0.003 RPS         |
| 1 h     | 1 h           | > 99.999 %               | 0.0003 RPS        |

### Pourquoi 30 s et pas plus

- **UX kill-switch** : en incident, un opérateur bascule un flag → les
  utilisateurs voient la correction en **≤ 30 s en p99** (acceptable pour
  un incident, invisible pour l'usage normal).
- **Rollout progressif** : passer de 10 % à 20 % se répercute en 30 s sans
  redémarrage de service.
- **Cohérence frontend / backend** : un flag ON en backend sera ON côté
  frontend dans la même fenêtre ≤ 30 s.

### Pourquoi 30 s et pas moins

- **Hit ratio** : à 100 req/s par instance, 1 miss / 30 s = **99.97 %** de
  cache hit. En dessous de 30 s, le gain marginal en fraîcheur coûte cher
  en appels GrowthBook.
- **Stampede** : TTL court → risque de N instances qui miss en même temps
  au rollover. À 30 s + jitter aléatoire ±5 s (à implémenter côté service),
  stampede évité.

### Exception : kill-switch ultra-critique

Pour un flag de type `kill-switch` (ex. `armageddon.response-cache-enabled`),
on peut ramener le TTL à **5 s** via le paramètre Spring
`faso.flags.ttl-seconds=5` pour l'environnement concerné. Les flags produits
restent à 30 s.

## 5. Attendus de performance

| Métrique                                | Cible    |
|-----------------------------------------|----------|
| Hit ratio KAYA (prod, par instance)     | > 95 %   |
| Hit ratio KAYA (prod, régime établi)    | > 99 %   |
| Latence `isOn()` p50                    | < 200 µs |
| Latence `isOn()` p99 (hit)              | < 2 ms   |
| Latence `isOn()` p99 (miss + GrowthBook)| < 50 ms  |
| GrowthBook RPS (par instance backend)   | < 0.1    |

## 6. Sécurité

- `GROWTHBOOK_API_KEY` : **Vault** (`kv faso/growthbook/sdk`), lue par Spring
  Cloud Vault. Jamais dans `application.yml`, jamais dans le repo.
- `JWT_SECRET`, `ENCRYPTION_KEY` GrowthBook : Vault (`kv faso/growthbook/core`).
- Pas d'appel direct frontend → GrowthBook (risque d'exfiltration de clef
  SDK). Toujours passer par le BFF `/api/flags`.
- Les attributs envoyés à GrowthBook n'incluent **jamais** de données
  sensibles (CNI, téléphone). Seuls `user_id`, `role`, `region`.

## 7. Fichiers associés

- `INFRA/growthbook/podman-compose.growthbook.yml` — stack compose
- `INFRA/growthbook/README.md` — workflow démarrage + seed flags
- `INFRA/growthbook/FEATURE-FLAGS-README.md` — gouvernance lifecycle
- `INFRA/poulets-platform/backend/src/main/java/bf/gov/faso/poulets/flags/` — SDK Java
- `INFRA/poulets-platform/frontend/src/app/core/flags/` — SDK Angular
- `INFRA/armageddon/armageddon-forge/src/feature_flag_filter.rs` — middleware gateway
- `INFRA/armageddon/armageddon-forge/src/feature_flags.rs` — struct de cache shared
