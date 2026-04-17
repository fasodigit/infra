<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<div align="center">

# FASO DIGITALISATION — Infrastructure Souveraine

**Plateforme numérique souveraine du Burkina Faso** · 100 % Rust / Java 21 / Angular 21 · **AGPL-3.0-or-later**

![License](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue)
![Status](https://img.shields.io/badge/Status-Active%20development-yellow)
![Rust](https://img.shields.io/badge/Rust-2021%20edition-orange)
![Java](https://img.shields.io/badge/Java-21%20Temurin-red)
![Angular](https://img.shields.io/badge/Angular-21-DD0031)

</div>

## Mission

Bâtir la pile numérique souveraine de l'État du Burkina Faso : 7 plateformes sectorielles
(État-civil, Hospital, E-Ticket, Vouchers, SOGESY, E-School, ALT-MISSION, FASO-Kalan, plateforme
pilote Poulets) partageant une infrastructure commune conçue, opérée, auditée par et pour le pays.

**Principe directeur** : chaque composant critique est remplacé par une alternative Rust native
dont le code source est intégralement lisible par les citoyens.

## Architecture souveraine

| Composant sovereign | Remplace (référence) | État |
|---------------------|----------------------|------|
| **KAYA** — base in-memory Rust (23 crates, RESP3, HTTP/3, WAL+snapshots, Pub/Sub, Functions Rhai HMAC, Geo, Probabilistic, JSON natif, Vector HNSW, TimeSeries Gorilla, Full-text Tantivy, Tiered RAM/NVMe) | Redis, DragonflyDB | 434 tests verts, parité Redis Stack complète |
| **ARMAGEDDON** — gateway sécurité Pentagon (19 crates : SENTINEL IPS/GeoIP/JA3/DLP, ARBITER WAF CRS, ORACLE ML, AEGIS Rego, AI prompt-injection, NEXUS brain, FORGE proxy, VEIL mask, WASM plugins, QUIC HTTP/3, mTLS SPIRE, xDS v3, LB×7, retry, WebSocket/L4, gRPC-Web, cache, Admin API) | Envoy, NGINX+, HAProxy | Vague 1 parité complète |
| **xds-controller** — control plane gRPC ADS v3 | Istio, Consul | Opérationnel |
| **Redpanda** — journal événementiel RAFT | Kafka | Intégré Schema Registry Buf |
| **YugabyteDB** — stockage durable ACID distribué | Cockroach, Spanner | Intégré |
| **ORY Kratos + Keto** — identité + permissions | Okta, Auth0 | Intégré |
| **auth-ms / poulets-api / notifier-ms** — microservices Java 21 Spring Boot | — | Intégrés, mTLS, JWT ES384, OpenAPI |
| **BFF Next.js 16 (Bun)** + **Angular 21** | — | Design system sovereign |

## Écosystème de sous-projets (7+1 pilote)

| Projet | Rôle | RPO cible | RTO cible |
|--------|------|-----------|-----------|
| **ÉTAT-CIVIL** | Actes naissance / mariage / décès | 0 | 15 min |
| **HOSPITAL** | Dossier médical partagé | 0 | 10 min |
| **E-TICKET** | Transport multi-compagnies | 1 s | 15 min |
| **VOUCHERS** | Intrants agricoles | 1 s | 30 min |
| **SOGESY** | Actes administratifs | 500 ms | 10 min |
| **E-SCHOOL** | Écoles privées agréées | 1 s | 1 h |
| **ALT-MISSION** | Ordres de mission | 1 s | 1 h |
| **FASO-KALAN** | E-learning universitaire | 1 s | 1 h |
| **Poulets** (pilote) | Marketplace éleveurs ↔ clients | 1 s | 1 h |

## Structure du dépôt

```
INFRA/
├── kaya/                # Base in-memory souveraine (Rust, 23 crates)
├── armageddon/          # Gateway sécurité (Rust, 19 crates)
├── xds-controller/      # Control plane xDS v3 (Rust, 5 crates)
├── auth-ms/             # Authentification JWT/JWKS (Java 21)
├── poulets-platform/    # Pilote : backend Java + BFF Next.js + Angular
├── notifier-ms/         # Notifier mail (Java 21, Redpanda consumer)
├── ory/                 # Configs Kratos + Keto
├── docker/compose/      # Stack podman-compose complète
├── docs/v3.1-souverain/ # Guide architectural + matrice RPO/RTO + Schema Registry Protobuf
├── observability/       # Grafana dashboards, Prometheus rules, Alertmanager, SLOs Sloth
├── chaos/               # Chaos Mesh experiments nightly
├── load-testing/        # k6 scenarios CI
├── synthetic-monitoring/# Playwright prod monitoring
├── spire/               # SPIRE server + agent + rotation SVID 24h
├── growthbook/          # Feature flags self-hosted + cache KAYA
├── scripts/             # Outils : SPDX auto, WAL replay, postmortem bot
└── .github/workflows/   # CI : Rust / Java / Frontend / Docker scan / SBOM / load / chaos / release
```

## Démarrage rapide

```bash
# Prérequis : podman, podman-compose, Rust 1.83+, Java 21, Node 22 + Bun 1.2
git clone https://github.com/fasodigit/infra
cd infra/INFRA/docker/compose

# 1. Générer les secrets locaux (jamais commités — voir secrets/.gitignore)
bash scripts/init-secrets.sh

# 2. Démarrer la stack complète (11 containers)
podman-compose up -d

# 3. Vérifier la santé
curl http://localhost:8080/api/poulets/health  # ARMAGEDDON → poulets-api
podman exec faso-kaya kaya-cli ping            # KAYA RESP3 → PONG

# 4. UI
open http://localhost:4801  # Angular frontend
open http://localhost:4800  # BFF Next.js
open http://localhost:3000  # Grafana (observability stack)
```

## Contribuer

1. Lire [`CONTRIBUTING.md`](CONTRIBUTING.md) et [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
2. Ouvrir une issue avant toute PR non triviale
3. Conventional commits obligatoires (bloqués par hook commitlint)
4. SPDX header AGPL-3.0 injecté automatiquement (pre-commit)
5. Tests + `cargo clippy -D warnings` + `mvn verify` + `bun run lint` doivent passer
6. Les revues de sécurité sont ouvertes publiquement — voir [`SECURITY.md`](SECURITY.md)

## Philosophie

- **Souveraineté** : remplacement progressif des dépendances externes critiques par des alternatives maîtrisées. AGPL-3.0 garantit que toute évolution reste partagée avec la communauté.
- **Transparence** : code publié en continu. Les audits sont les bienvenus — gouvernement, université, société civile, chercheurs.
- **Mobile-first / offline-first** : les utilisateurs terrain au Burkina Faso opèrent souvent sur réseau instable ; chaque UI est conçue pour fonctionner même après une coupure.
- **Multilingue** : français officiel + langues nationales (Mooré, Dioula, Fulfulde) en cours d'intégration.

## Communauté

- Issues publiques : https://github.com/fasodigit/infra/issues
- Discussions : https://github.com/fasodigit/infra/discussions
- Sécurité : voir [`SECURITY.md`](SECURITY.md) (contact chiffré + bug bounty)

## Licence

Licensed under the [**GNU AGPL v3 or later**](LICENSE).

Toute modification déployée comme service réseau doit être partagée avec ses utilisateurs —
c'est la garantie juridique de la souveraineté numérique.

---

<div align="center">

**FASO DIGITALISATION** · Ouagadougou · Burkina Faso · 2026

*« La souveraineté numérique se construit ligne de code après ligne de code. »*

</div>
