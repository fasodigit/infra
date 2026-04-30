<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Ultraplan TERROIR — module digitalisation coopératives agricoles BF -->

# /ultraplan — Module TERROIR

**Date** : 2026-04-30
**Statut** : à valider (gate avant P0)
**Périmètre** : digitalisation chaîne de valeur coton/sésame/karité/anacarde au Burkina Faso (M1) puis 7 pays Afrique de l'Ouest (M5+)
**Objectif business** : conformité EUDR (UE 2023/1115), traçabilité, paiements producteurs, registre coopératives, marché du carbone
**Scale cible** : 20 000+ coopératives × 50-500 producteurs ≈ 2-10M producteurs
**Durée totale** : 76 semaines (≈ 18 mois) en 7 phases (P0-P6)
**Mode** : agent-driven (Claude orchestre, utilisateur valide aux gates)
**Repo** : monorepo `INFRA/terroir/`

---

## 1. Décisions verrouillées (Q1-Q16 + P1-P4 + ADRs)

### Architecture
- **ADR-001** : RN+Expo, EAS Update **self-hosted** (souverain), keystore APK dans **Vault**, submission PlayStore par team FASO.
- **ADR-002** : sync hybrid CRDT/LWW/Append-only/ACID — table complète au §3.
- **ADR-003** : USSD multi-provider, mais P0-P2 uniquement **`terroir-ussd-simulator`** local (dev/test) ; intégration Hub2/AT/Twilio reportée à P3+ avec décision souveraineté ferme à ce moment-là.
- **ADR-004** : DDS modèle interne stable + mappers versionnés par schéma EU.
- **ADR-005** : pgcrypto + **Vault Transit envelope** (DEK per-record AES-256-GCM, KEK `terroir-pii-master` rotation 90j auto). Vault Transit à activer en **P0**.
- **ADR-006** : schema-per-tenant `terroir_t_<slug>` + Keto ABAC + RLS. **20 000+ tenants** cible long-terme → architecture optimisée pgbouncer/pg_partman dès P0.

### Données & sync (Q1 validé)
40 entités mappées (cf. §3). Producteurs offline = LWW pour scalaires, CRDT pour géom/notes, append-only pour transactions, ACID pour comptabilité/contrats.

### Identité & autorisation
- Agent terrain : **JWT Kratos émis online**, validité 14j sliding (Q2). Revocation : KAYA flag `auth:agent:revoked:{userId}` checké à chaque sync.
- Producteur USSD : **OTP 8 chiffres SMS**, anonymous session liée MSISDN, **re-enroll obligatoire** sur changement numéro (Q5).
- Multi-tenancy : Keto namespace `Tenant`/`Cooperative` ABAC + RLS Postgres (Q4).

### Compliance
- Audit : **schema isolé `audit_t_<slug>.audit_log` par tenant**, append-only, trigger DB (Q4).
- EUDR cut-off 2020-12-31 ; parcelles post-2020 en zone déforestée → workflow **autorité-BF** (Q6).
- DDS signée par **clé EORI exportateur** dans Vault PKI (Q13).

### Intégrations
- Mobile money : refactor `shared/mobile-money-lib` extrait de poulets-platform (Q9).
- Service mesh : tout via **ARMAGEDDON public route** `/api/terroir/*` (Q11).
- Tenant onboarding : self-service par exportateur (UI + API + Flyway auto ≤ 5min) (Q12).
- Buyer portal : **invitation-only par exportateur** (Q14).
- Sync conflict arbitrage : **gestionnaire union 24h** (Q15).
- DHIS2 export : reporté (Q16).

### Stratégie globale (P1-P4)
- Phasing P0→P6 validé.
- Implémentation agent-driven (P2).
- Admin-UI Phase 4.d : reprise après TERROIR P0 (6 sem.), puis TERROIR P1 (P3).
- Monorepo `INFRA/terroir/` (P4).

---

## 2. Architecture cible — 12 services + dépendances

| # | Service | Lang | Port HTTP | Port gRPC | Statut M1 | Dépendances |
|---|---|---|---|---|---|---|
| 1 | `terroir-core` | Rust Axum | 8830 | 8730 | P1 | Postgres+PostGIS, KAYA, Kratos JWT, Keto, Redpanda, audit-lib |
| 2 | `terroir-eudr` | Rust | 8831 | 8731 | P1 | terroir-core (gRPC), Hansen GFC mirror MinIO, JRC TMF mirror, Vault PKI |
| 3 | `terroir-payment` | Java Spring Boot | 8832 | — | P2 | terroir-core, mobile-money-lib, Redpanda, KAYA idempotency |
| 4 | `terroir-mobile-bff` | Rust | 8833 | — | P1 | terroir-core (gRPC), KAYA, sync engine CRDT |
| 5 | `terroir-ussd` | Rust | 8834 | — | P3 | terroir-core, terroir-ussd-simulator (P0-P2), notifier-ms |
| 6 | `terroir-ussd-simulator` | Rust | loopback | — | P0 | mock provider Hub2/AT/Twilio API surface |
| 7 | `terroir-buyer` | Rust | 8835 | — | P3 | terroir-core, terroir-eudr, Vault PKI signature |
| 8 | `terroir-payment-actuator` | (Spring) | 9004 | — | P2 | (loopback Spring actuator) |
| 9 | `terroir-admin` | Rust | 9904 | — | P0 | (loopback admin API : feature flags, tenant onboarding, debug) |
| 10 | `terroir-web-admin` | React (Vite) | 4810 | — | P1 | terroir-core via ARMAGEDDON :8080 |
| 11 | `terroir-buyer-portal` | Next.js 16 | 4811 | — | P3 | terroir-buyer via ARMAGEDDON, Vault PKI download endpoint |
| 12 | `terroir-mobile` (RN+Expo) | TypeScript | — | — | P1 | terroir-mobile-bff via ARMAGEDDON, Yjs CRDT, Expo SecureStore |

**Topology** : Browser/Mobile → ARMAGEDDON :8080 → BFF/services. Inter-services via gRPC mesh (mTLS SPIRE). Audit-lib injecté dans chaque service Rust + Java.

---

## 3. Entity × sync strategy (Q1 validé — référence pour migrations)

| Entité | Module | Strat | Stockage |
|---|---|---|---|
| `cooperative` (tenant) | shared | ACID | `terroir_shared.cooperative` |
| `producer` profile | M1 | LWW | `terroir_t_<slug>.producer` + Vault Transit DEK PII |
| `household` composition | M1 | CRDT | `terroir_t_<slug>.household` (Yjs doc en JSONB) |
| `parts_sociales` | M1 | ACID | `terroir_t_<slug>.parts_sociales` |
| `parcel` metadata | M2 | LWW | `terroir_t_<slug>.parcel` |
| `parcel_polygon` (geom) | M2 | CRDT (Yjs) | `terroir_t_<slug>.parcel_polygon` (PostGIS + Yjs doc) |
| `agronomy_note` | M2 | CRDT (text) | `terroir_t_<slug>.agronomy_note` |
| `parcel_eudr_validation` | M3 | Append-only | `terroir_t_<slug>.eudr_validation` |
| `dds` | M3 | ACID | `terroir_t_<slug>.dds` |
| `dds_submission` | M3 | Append-only | `terroir_t_<slug>.dds_submission` |
| `hansen_check`/`jrc_check` cache | M3 | Append-only | `terroir_shared.geo_check_cache` (versioned) |
| `evidence_archive` | M3 | Append-only | MinIO `terroir-evidence-<slug>` (WORM) |
| `harvest_lot` | M4 | Append-only | `terroir_t_<slug>.harvest_lot_event` |
| `sub_lot` lineage | M4 | Append-only | `terroir_t_<slug>.sub_lot_event` |
| `weighing` / `qr_code` | M4 | Append-only | `terroir_t_<slug>.weighing_event` |
| `harvest_approval_workflow` state | M4 | LWW | `terroir_t_<slug>.harvest_workflow` |
| `input_catalog` | M5 | ACID | `terroir_shared.input_catalog` |
| `input_distribution` | M5 | LWW | `terroir_t_<slug>.input_distribution` |
| `input_credit`/`input_repayment` | M5 | Append-only | `terroir_t_<slug>.input_ledger_event` |
| `payment_order` | M6 | ACID | `terroir_t_<slug>.payment_order` + KAYA `terroir:idempotent:payment:{orderId}` |
| `payment_transaction` | M6 | Append-only | `terroir_t_<slug>.payment_event` |
| `reconciliation_record` | M6 | Append-only | `terroir_t_<slug>.reconciliation_event` |
| `sms_notification_log` | M6 | Append-only | `terroir_t_<slug>.notification_event` |
| `accounting_journal` (SYSCOHADA) | M7 | ACID | `terroir_t_<slug>.accounting_journal` |
| `account_balance` | M7 | ACID | `terroir_t_<slug>.account_balance` |
| `lot_listing` (marketplace) | M8 | LWW | `terroir_t_<slug>.lot_listing` |
| `contract` | M8 | ACID | `terroir_t_<slug>.contract` + Vault PKI X.509 |
| `escrow_account` balance | M8 | ACID | `terroir_t_<slug>.escrow_account` |
| `signature_event` | M8 | Append-only | `terroir_t_<slug>.signature_event` |
| `indicator_value` (M&E) | M9 | Append-only | `terroir_shared.indicator_value` |
| `donor_aggregate` view | M9 | ACID (matview) | ClickHouse `terroir_donor_*` |
| `mooc_module` | M10 | LWW | `terroir_shared.mooc_module` |
| `learner_progress` | M10 | Append-only | `terroir_t_<slug>.learner_progress_event` |
| `credit_application` | M11 | ACID | `terroir_t_<slug>.credit_application` |
| `credit_score` | M11 | Append-only | `terroir_t_<slug>.credit_score_event` |
| `insurance_index_payout` | M11 | ACID | `terroir_t_<slug>.insurance_payout` |
| `carbon_credit` | M12 | Append-only | `terroir_t_<slug>.carbon_credit_event` |
| `mrv_event` | M12 | Append-only | `terroir_t_<slug>.mrv_event` |
| `audit_log` | shared | Append-only + trigger | `audit_t_<slug>.audit_log` |
| `settings` (config center) | shared | ACID + version CAS | `terroir_t_<slug>.settings` |
| `agent_terrain_session` | shared | LWW | `terroir_shared.agent_session` |

---

## 4. Phase P0 — Discovery & validation architecturale (6 semaines)

### Goal
Finaliser ADRs, scaffolds, infra de base, simulateur USSD, premiers Playwright squelettes. **Pas de logique métier**.

### Livrables

#### P0.1 — Bootstrap monorepo TERROIR
- `INFRA/terroir/` arborescence : `core/`, `eudr/`, `payment/`, `mobile-bff/`, `ussd/`, `ussd-simulator/`, `buyer/`, `admin/`, `web-admin/`, `buyer-portal/`, `mobile/`, `shared/`, `docs/`, `scripts/`
- Workspace Cargo `Cargo.toml` racine + crates Rust (skeleton avec `lib.rs` + `main.rs` + Containerfile vide)
- `podman-compose.terroir.yml` (override pour dev) — services stoppés par défaut, démarrables individuellement
- Header SPDX AGPL-3.0-or-later sur chaque fichier source

#### P0.2 — Vault Transit + PKI activation
- Script `INFRA/vault/scripts/configure-transit.sh` : enable secret engine `transit`, create key `terroir-pii-master` (AES-256-GCM, derived=true, exportable=false), policy rotation 90j auto.
- Script `INFRA/vault/scripts/configure-pki-terroir.sh` : intermediate CA `terroir-ca` issuant les certs EORI exportateur.
- Test : `vault write transit/encrypt/terroir-pii-master plaintext=...` → DEK envelope OK.

#### P0.3 — PostgreSQL extensions + multi-tenancy foundation
- Migration globale `INFRA/terroir/migrations/V001__shared_extensions.sql` : `CREATE EXTENSION postgis, pgcrypto, pg_partman, btree_gin`.
- Schema `terroir_shared` : tables transverses (cooperative, agent_session, geo_check_cache, indicator_value, mooc_module).
- Service `terroir-admin` (Rust :9904) endpoint `POST /admin/tenants` qui orchestre création schema `terroir_t_<slug>` + `audit_t_<slug>` + run Flyway sub-migrations.
- pgbouncer config : pool size dynamique, max_connections=500 PG, transaction pooling, prepared statements OFF (incompatible RLS).

#### P0.4 — Keto namespaces extensions
Étendre `INFRA/ory/keto/config/namespaces.ts` avec :
```typescript
class Tenant implements Namespace {
  related: { member: User[]; admin: User[]; agent_terrain: User[]; gestionnaire: User[]; exporter: User[] }
  permits = { /* per-tenant access checks */ }
}
class Cooperative implements Namespace { /* parent: Tenant */ }
class Parcel implements Namespace { /* parent: Cooperative, related: { editor, viewer } */ }
class HarvestLot implements Namespace { /* parent: Cooperative, related: { creator, approver } */ }
```
Seed tuples : 1 tenant pilote `t_pilot` + 1 SUPER-ADMIN `aminata` admin du tenant.

#### P0.5 — Topics Redpanda + schemas Avro
Script `INFRA/scripts/seed-redpanda-terroir-topics.sh` :
```
terroir.member.created/updated, terroir.parcel.created/updated, terroir.parcel.eudr.validated/rejected,
terroir.harvest.lot.recorded, terroir.payment.initiated/completed/failed, terroir.dds.generated/submitted/rejected,
terroir.tenant.provisioned, terroir.audit.event, terroir.sync.conflict.detected, terroir.sync.conflict.resolved,
terroir.ussd.session.started/ended, terroir.ussd.otp.sent/verified
```
Schemas Avro dans `INFRA/redpanda/schemas/terroir/*.avsc`.

#### P0.6 — `terroir-ussd-simulator` (Rust)
- HTTP server :loopback exposant l'API surface des 3 providers réels :
  - `POST /hub2/ussd/push` (mock Hub2)
  - `POST /africastalking/ussd/menu` (mock AT)
  - `POST /twilio/sms/send` (mock Twilio)
- Persiste les flows USSD en KAYA `terroir:ussd:simulator:session:{id}` TTL 30s.
- Fixtures : producteur signup, OTP 8 chiffres, paiement confirmation, vote AG.
- Endpoint `GET /admin/last-sms` pour Playwright (récupération OTP comme Mailpit).

#### P0.7 — `mobile-money-lib` extraction (P0 prep, refactor en P2)
- Repérer dans poulets-platform les intégrations existantes (Orange Money / Wave / Moov / MTN).
- Documenter l'API actuelle dans `INFRA/shared/mobile-money-lib/README.md`.
- **Pas de refactor en P0** — juste l'inventaire + design de l'API cible.

#### P0.8 — Mobile RN+Expo bootstrap
- `INFRA/terroir/mobile/` : `expo init` (React Native, TypeScript, EAS Build).
- `eas.json` config self-hosted EAS Update (à mettre en place P0 fin) — fallback Expo public temporaire pour valider le pipeline.
- Storage : `expo-secure-store` pour JWT + Vault DEK ciphertext local.
- Lib CRDT : `yjs` + `y-indexeddb-react-native` (custom adapter SQLite Expo).

#### P0.9 — ARMAGEDDON routes terroir
Étendre la config xDS ARMAGEDDON :
```
/api/terroir/*  → cluster terroir-core
/api/terroir/eudr/*  → cluster terroir-eudr
/api/terroir/mobile-bff/*  → cluster terroir-mobile-bff
```
Filter ext_authz inline → Keto check `Tenant`/`Cooperative` namespace.

#### P0.10 — Playwright scaffolds (CLAUDE.md §11)
Suite cible `INFRA/poulets-platform/e2e/tests/19-terroir/` (créée en P0.I) :
- `terroir-tenant-provisioning.spec.ts` (P0.C)
- `terroir-ussd-simulator-roundtrip.spec.ts` (P0.F)
- `terroir-vault-transit-encrypt-decrypt.spec.ts` (P0.B)
- `terroir-keto-tenant-namespace.spec.ts` (P0.D)

Fixtures associées : `INFRA/poulets-platform/e2e/fixtures/terroir/` :
- `tenant-admin-client.ts`, `ussd-simulator-client.ts`,
  `vault-transit-client.ts`, `keto-client.ts`.

> **Statut 2026-04-30** : Les 4 specs sont **écrites en P0.I (scaffolds)
> mais NON encore exécutées**. Leur exécution est portée par **P0.J
> cycle-fix** qui doit d'abord amener Vault Transit + terroir-admin +
> ussd-simulator + Keto namespaces au statut GREEN. Cf. CLAUDE.md §10
> (« cycle-fix AVANT E2E »).

Ces specs valident l'infra avant de commencer P1 modules.

#### P0.11 — Documentation
- ADRs déjà rédigés ; pas de modifs en P0 sauf feedback intégré.
- `INFRA/terroir/docs/RUNBOOK-P0.md` : ordre de bootstrap (Vault Transit → migrations → Keto seed → ussd-simulator → mobile boilerplate).

### Acceptance P0
- [ ] `cargo check --workspace` zero warning sur tous crates terroir-*.
- [ ] `terroir-admin` :9904 démarre, endpoint `POST /admin/tenants {slug:"t_pilot"}` crée le schema + audit schema en < 5 min, vérifié Flyway.
- [ ] Vault `vault write transit/encrypt/terroir-pii-master` retourne ciphertext.
- [ ] `terroir-ussd-simulator` répond aux 3 endpoints mock + produit OTP capturable.
- [ ] Keto namespaces `Tenant`/`Cooperative`/`Parcel`/`HarvestLot` enregistrés ; 1 tuple seed visible.
- [ ] 4 specs Playwright P0 passent (tenant provisioning, ussd, vault, keto).

### Effort P0
1 ETP Rust (terroir-core + ussd-simulator + admin) + 0.5 ETP DevOps (Vault, Keto, ARMAGEDDON config) + 0.5 ETP Mobile (RN bootstrap) = **2 ETP × 6 sem** (en agent-driven : 8-10 agents séquentiels/parallèles selon les sub-phases).

### Gate P0 → Admin-UI Phase 4.d
- Validation utilisateur sur :
  - 4 specs Playwright P0 GREEN
  - Vault Transit + PKI fonctionnels
  - Tenant provisioning < 5min
- Si OK → bascule vers **Admin-UI Phase 4.d** (33 specs Playwright admin sur stack GREEN actuelle), puis retour TERROIR P1.

---

## 5. Pause — Admin-UI Phase 4.d (entre P0 et P1)

Suite à la décision P3, on intercale Phase 4.d après P0 :

- Préparer fixtures `actors.ts` avec les SUPER-ADMIN seedés (Aminata, Souleymane).
- Créer ou enrichir 33 specs sous `tests-e2e/18-admin-workflows/`.
- Run cycle-fix incrémental si bugs.
- Une fois admin-UI validée GREEN → retour TERROIR P1.

---

## 6. Phase P1 — MVP modules 1-3 (12 semaines)

### Goal
Première version ROI : 500 producteurs enregistrés, 100 parcelles validées EUDR, 100 DDS soumises TRACES NT.

### Modules
1. **Registre membres** (terroir-core + web-admin + mobile)
2. **Cartographie parcelles** (terroir-core + mobile CRDT)
3. **Conformité EUDR** (terroir-eudr + Hansen mirror + DDS generation)

### Livrables

#### P1.1 — terroir-core (Rust Axum :8830)
- Migrations `V100__producer.sql`, `V101__parcel.sql`, `V102__household.sql` (par tenant) avec colonnes Vault Transit DEK pour PII.
- Endpoints REST :
  - `POST /producers` / `GET /producers` / `PATCH /producers/{id}` / `DELETE /producers/{id}`
  - `POST /parcels` / `GET /parcels` / `POST /parcels/{id}/polygon` (CRDT update)
  - `POST /households` (CRDT)
- gRPC service `terroir.core.v1.CoreService` (proto fichier `INFRA/terroir/proto/core.proto`).
- Audit-lib : chaque write → `audit_t_<slug>.audit_log` + publish `terroir.member.*` Redpanda.
- KAYA : `terroir:cache:producer:{id}` TTL 5min ; `terroir:idempotent:{key}` TTL 24h.

#### P1.2 — Vault Transit envelope encryption
- Service `crypto/PiiEncryptionService` (Rust) : encrypt/decrypt via Vault Transit, DEK cached KAYA `terroir:dek:cache:{kid}` TTL 1h.
- Champs PII chiffrés sur `producer` : `nin`, `phone`, `photo_url`, `gps_domicile_lat`, `gps_domicile_lon`.
- Migration `V103__pii_encrypted_columns.sql` ajoute colonnes `<field>_encrypted bytea` + `<field>_dek_kid text`.

#### P1.3 — terroir-eudr (Rust :8831)
- Validation algorithme :
  1. Reçoit `parcel_id` + `polygon GeoJSON`.
  2. Cache check KAYA `terroir:eudr:result:{polygon_hash}` TTL 30j.
  3. Si miss → query Hansen GFC mirror MinIO + JRC TMF mirror, calcule overlap.
  4. Si overlap zone déforestée post-2020 → status `REJECTED` + raison + workflow `escalate_authority_bf`.
  5. Sinon → status `VALIDATED` + génère DDS draft.
- Endpoints :
  - `POST /eudr/validate {parcel_id}` → `{status, evidence_url, dds_draft_id}`
  - `POST /eudr/dds/{id}/sign` → signature Vault PKI EORI exportateur
  - `POST /eudr/dds/{id}/submit` → POST async vers TRACES NT API
- Topic `terroir.dds.generated/submitted/rejected`.

#### P1.4 — Hansen GFC + JRC TMF mirrors
- Script `INFRA/scripts/sync-hansen-gfc.sh` : download v1.11 dataset (~50GB) vers MinIO `geo-mirror/hansen/v1.11/`.
- Script `INFRA/scripts/sync-jrc-tmf.sh` idem.
- Cron quotidien check version upstream, alerte si nouvelle version.
- License attribution dans `INFRA/terroir/docs/LICENSES-GEO.md`.

#### P1.5 — terroir-mobile-bff (Rust :8833)
- Endpoints orientés mobile : pagination, payload léger, batch sync.
- Sync engine CRDT : reçoit Yjs updates depuis app mobile, merge, broadcast aux autres clients connectés.
- WebSocket `/sync` (proxy via ARMAGEDDON) pour push CRDT updates.

#### P1.6 — terroir-mobile (RN+Expo)
- 6 écrans P1 : Login / Profil agent / Liste producteurs / Création producteur (CNIB capture, GPS, photo) / Liste parcelles / Création parcelle (polygone GPS sur carte MapLibre).
- Storage offline : SQLite Expo + Yjs IndexedDB-equivalent.
- Sync : periodic push every 60s online, queue offline.
- EAS Build target Android (API ≥ 24 / Android 7+) — couvre Tecno Spark Go.
- APK target ≤ 25 MB.

#### P1.7 — terroir-web-admin (React Vite :4810)
- Dashboard KYC validation producteurs.
- Carte parcelles interactive (Leaflet + GeoJSON).
- Detail parcelle avec status EUDR + evidence.
- Export DDS preview (PDF).
- Auth : Kratos session via ARMAGEDDON.

#### P1.8 — Specs Playwright P1 (CLAUDE.md §11)
Suite `tests-e2e/19-terroir/` :
- `terroir-producer-create-with-pii-encryption.spec.ts` — création + assert champs chiffrés en DB
- `terroir-parcel-polygon-crdt-merge.spec.ts` — 2 agents éditent, merge convergent
- `terroir-eudr-validation-happy-path.spec.ts` — parcelle clean → VALIDATED
- `terroir-eudr-validation-deforested.spec.ts` — parcelle post-2020 zone Hansen → REJECTED + workflow autorité
- `terroir-dds-generation-and-submission.spec.ts` — DDS PDF généré + signé Vault PKI + soumis TRACES NT (mock)
- `terroir-agent-offline-sync-roundtrip.spec.ts` — agent crée 50 producteurs offline, retour réseau, sync OK
- `terroir-jwt-revocation-on-sync.spec.ts` — agent révoqué pendant offline → sync rejected
- `terroir-tenant-isolation.spec.ts` — agent tenant A ne voit pas données tenant B (Keto + RLS)

### Acceptance P1
- [ ] 500 producteurs enregistrés (200 coop pilote + 300 test) ; validity ≥ 80%.
- [ ] 100 parcelles validées EUDR avec accuracy ≥ 95% vs ground truth agronome.
- [ ] 100 DDS générées + soumises TRACES NT, ≥ 99% accepted.
- [ ] App RN APK ≤ 25 MB, cold start ≤ 3s sur Tecno Spark Go.
- [ ] Sync 50 livraisons offline ≤ 2 min en EDGE.
- [ ] 8 specs Playwright P1 GREEN.
- [ ] Pen test RSSI : cross-tenant isolation OK, PII encryption OK, Vault rotation OK.

### Effort P1
2 ETP Rust (core + eudr + mobile-bff) + 1 ETP Frontend RN + React + 0.5 ETP DevOps (Hansen mirror, Keto/Vault prod) + 0.5 ETP SME EUDR (validation accuracy) = **4 ETP × 12 sem**. Agent-driven : ~25-30 agents séquentiels.

### Gate P1 → P2
- 500 producteurs + 100 DDS validés.
- Exportateur pilote signe contrat SaaS production (LOI → contrat).

---

## 7. Phase P2 — Récolte + intrants + paiement (10 semaines)

### Modules
4. Traçabilité récolte (terroir-core)
5. Gestion intrants (terroir-core)
6. Paiements mobile money (terroir-payment Java Spring Boot)

### Livrables clés

#### P2.1 — Refactor `mobile-money-lib`
- Extraction depuis poulets-platform vers `INFRA/shared/mobile-money-lib/` (Maven module Java).
- API : `MobileMoneyClient.requestPayment(provider, msisdn, amount, idempotencyKey, callbackUrl)`.
- Providers : Orange Money BF, Wave, Moov, MTN. Credentials Vault `faso/mobile-money/<provider>/{api_key,merchant_id}`.
- Test : `terroir-payment` ET `poulets-api` consomment la même lib.

#### P2.2 — terroir-payment (Java Spring Boot :8832)
- Endpoints : `POST /payments` (idempotent KAYA), `GET /payments/{id}`, `POST /payments/{id}/confirm` (callback provider).
- Reconciliation : nightly batch + real-time CDC.
- Topics : `terroir.payment.initiated/completed/failed`.
- Notifier-ms consume → SMS/USSD producteur via simulator (P2) ou providers réels (P3+).

#### P2.3 — Harvest tracking (terroir-core)
- Tables event-sourced : `harvest_lot_event`, `sub_lot_event`, `weighing_event`.
- Endpoints mobile : `POST /harvest/lots`, `POST /harvest/lots/{id}/weighings`.
- Scanner balance BLE (Bluetooth Low Energy) côté mobile RN.
- QR codes générés côté serveur, scannables au point d'achat.

#### P2.4 — Intrants management
- `input_catalog` (semences, engrais, vaccins, alevins) référentiel partagé.
- `input_distribution` per tenant + crédit producteur.
- Workflow remboursement : lié à payment ledger.

#### P2.5 — Specs Playwright P2
- `terroir-harvest-lot-creation-offline.spec.ts`
- `terroir-payment-mobile-money-idempotent.spec.ts`
- `terroir-payment-double-spend-rejected.spec.ts`
- `terroir-payment-reconciliation-batch.spec.ts`
- `terroir-input-distribution-credit-tracking.spec.ts`
- `terroir-sms-payment-confirmation.spec.ts`

### Acceptance P2
- [ ] 1 campagne agricole complète (saisie → récolte → paiement) sans incident.
- [ ] 50M CFA paiements traités, < 0.1% loss.
- [ ] P95 latence mobile money ≤ 8s.
- [ ] 0 double-payment (1000 replays testés).
- [ ] SMS delivery ≥ 98%.
- [ ] 6 specs Playwright P2 GREEN.

### Gate P2 → P3
ARR 50 k€ (clients coops payants + donors).

---

## 8. Phase P3 — Marketplace + reporting + USSD providers réels (8 semaines)

### Modules
7. Comptabilité OHADA (terroir-core)
8. Marketplace acheteurs (terroir-buyer Rust :8835)
9. Reporting bailleurs (ClickHouse + terroir-web-bailleur React)

### Livrables clés

#### P3.1 — Décision USSD providers
**Gate spécifique** : à l'entrée de P3, validation utilisateur sur :
- (a) Intégrer Hub2 + Africa's Talking (avec clauses contractuelles "données restent Afrique")
- (b) Rester sur simulator + signer accord direct ARCEP/ANATEL pour shortcode opérateur BF (long terme souverain)
- (c) Hybride : Hub2/AT en prod + simulator en dev/CI

Si (a) ou (c) → implémentation `terroir-ussd` (Rust :8834) avec adapter pattern + fallback logic + healthcheck KAYA.

#### P3.2 — terroir-buyer (Rust :8835)
- Endpoints : `GET /lots` (public catalog), `POST /lots/{id}/contract` (escrow + Vault PKI signature), `GET /dds/{id}/download` (PDF signé).
- Auth invitation-only : token JWT signé par exportateur, scope `buyer.<exporter_id>`.

#### P3.3 — terroir-buyer-portal (Next.js 16 :4811)
- Public listings (SEO).
- Acheteur signup invitation-only (token email).
- Détail lot + contract signature workflow.
- DDS download (signed JWT timestamped pour non-repudiation).

#### P3.4 — Reporting bailleurs
- ClickHouse cluster (peut être 1 node dev).
- Materialized views ingérant Redpanda CDC `terroir.*`.
- Dashboards Grafana (terroir-bailleur-bm.json, terroir-bailleur-afd.json).
- Export JSON/CSV agrégats anonymisés (k-anon ≥ 5).

#### P3.5 — Comptabilité SYSCOHADA
- Plan comptable BF dans `terroir_shared.account_chart`.
- Journaux par tenant.
- Export Sage/Tompro (formats CSV).

#### P3.6 — Specs Playwright P3
- `terroir-buyer-invitation-signup.spec.ts`
- `terroir-buyer-contract-signature-vault-pki.spec.ts`
- `terroir-buyer-dds-download-signed.spec.ts`
- `terroir-bailleur-dashboard-real-time.spec.ts`
- `terroir-syscohada-export-sage.spec.ts`
- `terroir-ussd-failover-hub2-to-at.spec.ts` (si décision (a)/(c))

### Acceptance P3
- 1 contrat acheteur signé (BM-financed coop vend à exportateur).
- 5 coops non-pilote déployées via tenant onboarding sans incident.
- Break-even unit economics 1 coop.
- Bailleurs rapports daily autonomous.

### Gate P3 → P4
Unit economics validés.

---

## 9. Phase P4 — Scale 5-10 coops + SRE (12 semaines)

### Goal
10 000 producteurs live, 5-10 coops payantes, SLO 99.5%.

### Livrables
- Runbooks incident response, deployment, rollback, DR.
- Pen test externe.
- Backup per-tenant testé (pg_dump par schema, restore time).
- Multi-tenancy mature : 50+ schemas, pgbouncer tuned.
- Observabilité : alerts Prometheus + Sloth SLO files `terroir-*.slo.yaml`.
- Support tier-1 training docs.

### Specs Playwright P4
- `terroir-scale-50-tenants-no-degradation.spec.ts` (load test)
- `terroir-disaster-recovery-postgres-restore.spec.ts`
- `terroir-slo-99-5-burn-rate-alert.spec.ts`

---

## 10. Phase P5 — Formation + crédit + assurance (12 semaines)

### Modules
10. Formation MOOC
11. Crédit scoring + assurance climatique

### Livrables clés
- Plateforme MOOC (vidéo offline-first, quiz, certificats blockchain optionnel).
- Service `terroir-credit` (Java Spring Boot) : scoring KYC + ML model (Python sidecar).
- Intégration ARC (African Risk Capacity) pour assurance index paramétrique.
- Partenariat SFD (Small Finance Deed) BF.

### Specs Playwright P5
- `terroir-mooc-offline-video-playback.spec.ts`
- `terroir-credit-scoring-application.spec.ts`
- `terroir-insurance-payout-trigger.spec.ts`

---

## 11. Phase P6 — Marché du carbone + multi-pays (16 semaines)

### Module
12. MRV + Verra/Gold Standard credit generation + tokenisation Blockchain (optional)

### Livrables clés
- Service `terroir-carbon` (Rust :8836 — réservation port à venir).
- Intégration Verra registry API.
- Réplication multi-pays : CI, SN, ML, BJ, TG, NE, GH (1 schema-per-pays-per-coop).
- Optionnel : tokenisation ERC-20 sur chaîne publique souveraine (Polygon zkEVM ?).

### Specs Playwright P6
- `terroir-carbon-mrv-event-recording.spec.ts`
- `terroir-carbon-credit-issuance-verra.spec.ts`
- `terroir-multi-country-tenant-isolation.spec.ts`

---

## 12. Assets shared à créer

| Asset | Localisation | Phase | Description |
|---|---|---|---|
| `mobile-money-lib` | `INFRA/shared/mobile-money-lib/` | P2 | Java module, providers Orange/Wave/Moov/MTN |
| `terroir-ussd-simulator` | `INFRA/terroir/ussd-simulator/` | P0 | Rust mock 3 providers + KAYA state |
| `Vault Transit setup` | `INFRA/vault/scripts/configure-transit.sh` | P0 | Script idempotent |
| `Vault PKI terroir` | `INFRA/vault/scripts/configure-pki-terroir.sh` | P0 | Intermediate CA + EORI cert template |
| `PostGIS extension` | `INFRA/terroir/migrations/V001__shared_extensions.sql` | P0 | postgis, pgcrypto, pg_partman |
| `Keto namespaces` | `INFRA/ory/keto/config/namespaces.ts` | P0 | Tenant, Cooperative, Parcel, HarvestLot |
| `Hansen GFC mirror` | `INFRA/scripts/sync-hansen-gfc.sh` + MinIO | P1 | Cron sync, license attribution |
| `JRC TMF mirror` | `INFRA/scripts/sync-jrc-tmf.sh` + MinIO | P1 | Cron sync |
| `terroir-proto` | `INFRA/terroir/proto/*.proto` | P0-P6 | gRPC schemas |

---

## 13. Risques & mitigations

| Risque | Sévérité | Phase | Mitigation |
|---|---|---|---|
| 20k+ tenants schema-per-tenant intenable PG | HAUTE | P4 | pg_partman + shard PG horizontal en P5 si > 5k |
| Hansen/JRC datasets size 50GB+ | MOYENNE | P1 | MinIO mirror + delta sync + version pinning |
| EUDR validateur faux positifs | HAUTE | P1 | Ground-truth agronome 95% + appel autorité-BF |
| CRDT mobile complexité | MOYENNE | P1 | Yjs mature, fallback LWW si bug bloquant |
| Vault Transit latency PII (5-15ms × write) | BASSE | P1 | DEK cache KAYA TTL 1h |
| USSD providers downtime | HAUTE | P3 | Simulator fallback maintenu en CI |
| Mobile money provider double-billing | CRITIQUE | P2 | Idempotency KAYA + reconciliation CDC |
| RLS performance large dataset | MOYENNE | P4 | Index policies + partitioning per tenant |
| EORI certif compromission | CRITIQUE | P3 | Vault PKI rotation + revocation publique |
| Souveraineté FCM/APN si on switch push notifications | HAUTE | P5+ | WebSocket pattern (cf. ARCHITECTURE-SECURITE-COMPLETE §13) |

---

## 14. Séquencement agents par phase

### Pattern global
Chaque phase = 1 ultraplan local (court) → décomposé en streams parallèles → 5-10 agents `general-purpose` / `kaya-rust-implementer` / `database-internals-rust` / `distributed-systems-rust` selon le périmètre.

### Phase P0 — séquencement proposé
1. Agent A — Bootstrap monorepo + Cargo workspace + Containerfiles (general-purpose, 1 run)
2. Agent B — Vault Transit + PKI scripts (general-purpose, 1 run)
3. Agent C — PostgreSQL extensions + multi-tenancy foundation (database-internals-rust, 1 run)
4. Agent D — Keto namespaces + seed (general-purpose, 1 run)
5. Agent E — Topics Redpanda + Avro schemas (distributed-systems-rust, 1 run)
6. Agent F — terroir-ussd-simulator (general-purpose, 1 run)
7. Agent G — Mobile RN+Expo bootstrap (general-purpose, 1 run)
8. Agent H — ARMAGEDDON routes terroir + ext_authz Keto (distributed-systems-rust, 1 run)
9. Agent I — Specs Playwright P0 (4 specs, general-purpose, 1 run)
10. Agent J — Cycle-fix P0 (general-purpose, 1 run après tous les autres)

Total : 10 agents séquentiels (avec parallélisme partiel) = ~6 sem. en agent-driven.

### P1 → P6 : structure similaire, ~25-50 agents par phase selon scope.

---

## 15. Gates de validation utilisateur

| Gate | Phase | Décision attendue |
|---|---|---|
| **G0** | Avant P0 | Validation de cet ultraplan (toi, maintenant) |
| **G1** | Fin P0 | Specs P0 GREEN + Vault Transit OK + tenant provisioning < 5min |
| **G_admin** | Entre P0 et P1 | Admin-UI Phase 4.d 33 specs GREEN |
| **G2** | Fin P1 | 500 producteurs + 100 DDS validés + LOI exportateur signée |
| **G3** | Fin P2 | ARR 50k€ + 1 campagne complète sans incident |
| **G_ussd** | Entrée P3 | Décision USSD providers (Hub2/AT vs simulator vs hybride) |
| **G4** | Fin P3 | 5 coops déployées + unit economics OK |
| **G5** | Fin P4 | 10k producteurs + SLO 99.5% |
| **G6** | Fin P5 | MOOC actif + 1 crédit accordé + 1 assurance versée |
| **G7** | Fin P6 | 1 carbon credit émis Verra + 1 pays additionnel onboardé |

À chaque gate : agent reporte → user valide ou demande ajustement → next phase.

---

## 16. Annexes

### A. Glossaire
- **DDS** : Due Diligence Statement (déclaration de diligence raisonnable EUDR)
- **EUDR** : EU Deforestation Regulation 2023/1115
- **TRACES NT** : système EU de traçabilité commerce
- **Hansen GFC** : Global Forest Change dataset (Univ. Maryland)
- **JRC TMF** : Tropical Moist Forest dataset (EU Joint Research Centre)
- **EORI** : Economic Operators Registration and Identification (UE)
- **MRV** : Monitoring, Reporting, Verification (carbone)
- **CRDT** : Conflict-free Replicated Data Type
- **LWW** : Last-Write-Wins
- **SYSCOHADA** : référentiel comptable OHADA
- **MSISDN** : numéro de téléphone international
- **NIN** : Numéro d'Identification Nationale (CNIB pour BF)

### B. Références
- ADR-001 à ADR-006 : `INFRA/terroir/docs/adr/`
- PLAN-TERROIR : `INFRA/terroir/docs/PLAN-TERROIR.md`
- Spike EUDR : `INFRA/terroir/docs/eudr-validator-spike.md`
- Analysis pre-implementation : `INFRA/terroir/docs/ANALYSIS-PRE-IMPLEMENTATION-2026-04-30.md`
- Architecture sécurité (réutilisations) : `INFRA/docs/ARCHITECTURE-SECURITE-COMPLETE-2026-04-30.md`
- Règles projet : `INFRA/CLAUDE.md` (souveraineté §3, AGPL §4, sécurité §5, podman §1, ports §8, post-coding §9, cycle-fix avant E2E §10, **Playwright en miroir §11**)

### C. Décisions à reporter
- **P3 USSD providers** (Hub2/AT vs simulator vs hybride)
- **P5 partenariat SFD/ARC** (lequel exactement)
- **P6 blockchain** (Polygon zkEVM ou autre L2 souverain)

---

*Ultraplan TERROIR à valider — gate G0. Une fois validé, je crée la TaskList P0 et lance les agents.*
