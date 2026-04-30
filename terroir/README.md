<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# TERROIR — Module digitalisation coopératives agricoles BF

**Mission** : digitaliser la chaîne de valeur coton/sésame/karité/anacarde
au Burkina Faso (M1) puis 7 pays Afrique de l'Ouest (M5+) — conformité
EUDR, traçabilité, paiements producteurs, registre coopératives, marché
du carbone.

**Scale cible** : 20 000+ coopératives × 50-500 producteurs ≈ 2-10M
producteurs.

**Phasing** : 7 phases (P0 → P6) sur ≈ 18 mois — voir
[`docs/ULTRAPLAN-TERROIR-2026-04-30.md`](docs/ULTRAPLAN-TERROIR-2026-04-30.md).

**Statut actuel** : **Phase P0.A — Bootstrap monorepo** (foundation,
ce répertoire).

---

## 1. Vue d'ensemble — 12 services

| # | Service | Tech | Port HTTP | Port gRPC | Phase |
|---|---|---|---|---|---|
| 1 | `terroir-core` | Rust Axum + Tonic | 8830 | 8730 | P1 |
| 2 | `terroir-eudr` | Rust Axum + Tonic | 8831 | 8731 | P1 |
| 3 | `terroir-payment` | Java Spring Boot | 8832 | — | P2 |
| 4 | `terroir-mobile-bff` | Rust Axum | 8833 | — | P1 |
| 5 | `terroir-ussd` | Rust Axum | 8834 | — | P3 |
| 6 | `terroir-ussd-simulator` | Rust Axum | 1080 (loopback) | — | **P0** |
| 7 | `terroir-buyer` | Rust Axum | 8835 | — | P3 |
| 8 | `terroir-payment-actuator` | Spring | 9004 (loopback) | — | P2 |
| 9 | `terroir-admin` | Rust Axum | 9904 (loopback) | — | **P0** |
| 10 | `terroir-web-admin` | Vite React | 4810 | — | P1 |
| 11 | `terroir-buyer-portal` | Next.js 16 | 4811 | — | P3 |
| 12 | `terroir-mobile` | RN + Expo | (n/a) | — | P1 |

Ports réservés dans [`INFRA/port-policy.yaml`](../port-policy.yaml)
(plages `agri-services-http: 8830-8849`, `agri-services-grpc: 8730-8749`,
`admin-api: 9900-9999`, `frontend: 4800-4899`).

## 2. Arborescence

```
INFRA/terroir/
├── Cargo.toml                       # workspace racine
├── README.md                        # ce fichier
├── podman-compose.terroir.yml       # override stack TERROIR
│
├── core/                # crate Rust — registre membres + parcelles
├── eudr/                # crate Rust — validation EUDR + DDS
├── mobile-bff/          # crate Rust — BFF mobile agent terrain
├── ussd/                # crate Rust — gateway USSD/SMS (P3)
├── ussd-simulator/      # crate Rust — mock providers (P0/P1/P2)
├── buyer/               # crate Rust — portail acheteurs (P3)
├── admin/               # crate Rust — admin API loopback (P0)
│
├── payment/             # placeholder Java Spring Boot (P2 — README only)
├── web-admin/           # placeholder Vite React (P1 — README only)
├── buyer-portal/        # placeholder Next.js 16 (P3 — README only)
├── mobile/              # placeholder RN + Expo (P0.G bootstrap)
│
├── proto/               # gRPC schemas (.proto)
│
├── docs/                # ADRs + ULTRAPLAN + spike EUDR
│   ├── adr/             # ADR-001 → ADR-006
│   ├── ULTRAPLAN-TERROIR-2026-04-30.md
│   ├── PLAN-TERROIR.md
│   ├── ANALYSIS-PRE-IMPLEMENTATION-2026-04-30.md
│   └── eudr-validator-spike.md
│
└── scripts/
    ├── bootstrap-p0.sh    # orchestrateur Phase P0 (squelette)
    └── start-dev.sh       # démarre la stack en mode dev natif
```

## 3. Dépendances inter-services

```
terroir-mobile (RN+Expo)
        │ HTTPS
        ▼
ARMAGEDDON :8080  /api/terroir/mobile-bff/*
        │
        ▼
terroir-mobile-bff :8833
        │ gRPC
        ▼
terroir-core :8730
        ├── PostgreSQL+PostGIS (schema-per-tenant)
        ├── KAYA (cache + idempotency)
        ├── Kratos JWT (validation)
        ├── Keto (ABAC Tenant/Cooperative/Parcel)
        ├── Redpanda (terroir.member.*)
        ├── audit-lib (audit_t_<slug>.audit_log)
        └── Vault Transit (PII envelope encryption)

terroir-eudr :8831 ───gRPC────► terroir-core :8730
        ├── MinIO (Hansen GFC mirror)
        ├── MinIO (JRC TMF mirror)
        └── Vault PKI (signature DDS EORI)

terroir-payment :8832 (Java)
        ├── INFRA/shared/mobile-money-lib/ (P2 extraction)
        ├── Redpanda (terroir.payment.*)
        └── KAYA (terroir:idempotent:payment:{key})

terroir-buyer :8835 ───gRPC────► terroir-eudr :8731
                              └─► terroir-core :8730

terroir-admin :9904 (loopback)
        └── PostgreSQL (provisioning schemas terroir_t_<slug>)

terroir-ussd :8834 ──HTTP──► terroir-ussd-simulator :1080 (P0/P1/P2)
                       └─► Hub2/AT/Twilio (P3, gate G_ussd)
```

## 4. Démarrage

### 4.1 Mode containerisé (canonique)

```bash
cd INFRA/docker/compose
podman-compose -f podman-compose.yml \
               -f ../../terroir/podman-compose.terroir.yml \
               --profile terroir up -d

# Sur machine contributeur sans podman :
docker compose   -f podman-compose.yml \
                 -f ../../terroir/podman-compose.terroir.yml \
                 --profile terroir up -d
```

### 4.2 Mode dev natif (Rust direct)

```bash
cd INFRA/terroir
cargo build --workspace
bash scripts/start-dev.sh
# Logs : /tmp/terroir-*.log

# Healthchecks :
curl http://127.0.0.1:9904/health/ready    # terroir-admin
curl http://127.0.0.1:1080/health/ready    # terroir-ussd-simulator
curl http://127.0.0.1:8830/health/ready    # terroir-core
```

### 4.3 Bootstrap Phase P0 complet

```bash
bash INFRA/terroir/scripts/bootstrap-p0.sh
# Orchestre : Vault Transit → Postgres seed → Keto seed → Redpanda topics
#           → ussd-simulator → tenant pilote (t_pilot)
# (les sub-scripts sont implémentés par les autres streams P0.B/C/D/E/F)
```

## 5. Tests / lint / format

```bash
cd INFRA/terroir
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
cargo test --workspace
```

## 6. Stream P0 — découpe en 8 sous-phases parallèles

| Stream | Livrable | Statut |
|---|---|---|
| **P0.A** | Bootstrap monorepo + Cargo workspace + Containerfiles + override compose | **CE LIVRABLE** |
| P0.B | Vault Transit + PKI scripts | TODO |
| P0.C | PostgreSQL extensions + multi-tenancy foundation + `terroir-admin POST /admin/tenants` | TODO |
| P0.D | Keto namespaces (`Tenant`/`Cooperative`/`Parcel`/`HarvestLot`) + tuple seed | TODO |
| P0.E | Topics Redpanda + Avro schemas | TODO |
| P0.F | `terroir-ussd-simulator` fixtures (3 providers + KAYA + `/admin/last-sms`) | TODO |
| P0.G | Mobile RN+Expo bootstrap (EAS Build, expo-secure-store, Yjs CRDT) | TODO |
| P0.H | ARMAGEDDON routes terroir + ext_authz Keto | TODO |
| P0.I | Specs Playwright P0 (4 specs : tenant provisioning, ussd, vault, keto) | TODO |
| P0.J | Cycle-fix P0 (boucle stabilisation jusqu'à GREEN) | TODO |

## 7. Acceptance gate G1 (sortie P0)

- [ ] `cargo check --workspace` — zero warning sur tous crates `terroir-*` (✅ acquis P0.A).
- [ ] `terroir-admin POST /admin/tenants {slug:"t_pilot"}` crée schema + audit schema en < 5 min.
- [ ] Vault `vault write transit/encrypt/terroir-pii-master` retourne ciphertext.
- [ ] `terroir-ussd-simulator` répond aux 3 endpoints mock + produit OTP capturable.
- [ ] Keto namespaces `Tenant`/`Cooperative`/`Parcel`/`HarvestLot` enregistrés ; 1 tuple seed visible.
- [ ] 4 specs Playwright P0 passent.

## 8. Souveraineté

Stack 100% souveraine — voir [`INFRA/CLAUDE.md`](../CLAUDE.md) §3 :

- **KAYA** (in-memory DB) au lieu d'alternatives propriétaires.
- **ARMAGEDDON** (gateway/mesh) au lieu d'alternatives propriétaires.
- **xds-controller** au lieu d'un control plane Istio.

Exception conservée : Vault + Consul + Postgres + Temporal + Redpanda
(pas d'alternative Rust mature en 2026-04 ; évaluation `openbao` future).

## 9. Licence

[AGPL-3.0-or-later](../LICENSE) — © 2026 FASO DIGITALISATION, Burkina Faso.

Header SPDX obligatoire sur chaque fichier source (`.rs`, `.proto`,
`.toml` quand le format le permet, `.sh`, `.yml`, `.md`).

## 10. Références

- ULTRAPLAN : [`docs/ULTRAPLAN-TERROIR-2026-04-30.md`](docs/ULTRAPLAN-TERROIR-2026-04-30.md)
- Plan détaillé : [`docs/PLAN-TERROIR.md`](docs/PLAN-TERROIR.md)
- ADRs : [`docs/adr/`](docs/adr/)
- Spike validateur EUDR : [`docs/eudr-validator-spike.md`](docs/eudr-validator-spike.md)
- Pré-implémentation : [`docs/ANALYSIS-PRE-IMPLEMENTATION-2026-04-30.md`](docs/ANALYSIS-PRE-IMPLEMENTATION-2026-04-30.md)
- Mobile money inventaire : [`../shared/mobile-money-lib/README.md`](../shared/mobile-money-lib/README.md)
- Règles globales : [`../CLAUDE.md`](../CLAUDE.md)
- Port-policy : [`../port-policy.yaml`](../port-policy.yaml)
