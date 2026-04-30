Parfait. J'ai suffit d'informations. Produisons le rapport exhaustif maintenant.

---

# RAPPORT D'ANALYSE EXHAUSTIF — TERROIR × INFRA FASO
## Module TERROIR (digitalisation coopératives agricoles BF)

**Date:** 30 avril 2026  
**Auteur:** Claude Code (analyse)  
**Destinataire:** Lionel Traore (FASO DIGITALISATION)  
**Scope:** Cross-référence architecture TERROIR vs infra FASO existante  

---

## SECTION 1 — SYNTHÈSE FONCTIONNELLE TERROIR

### 1.1 Domaine métier et utilisateurs finaux

TERROIR digitalise la chaîne de valeur **coton, sésame, karité, anacarde** en Afrique de l'Ouest (BF, CI, SN, ML, BJ, TG, NE, GH), avec un focus initial Burkina Faso. Le problème core : les coopératives agricoles n'ont aucun outil pour tracer leurs récoltes, démontrer la conformité EUDR (Règlement UE 2023/1115 — proof de non-déforestation post-2020), et payer leurs producteurs rapidement. (PLAN-TERROIR.md §1)

**Cinq personas clés** parcourent le système :

1. **Agent collecteur terrain (agent_terrain)** : parcourt 7-14 jours en brousse, sans réseau, saisit 50+ livraisons quotidiennes, photo CNIB + GPS parcelle + poids. Majorité analphabète tech. *JTBD* : sync delta-encoded offline → backend au retour réseau, aucune perte donnée. (§2.1)

2. **Producteur rural (~70% sans smartphone)** : reçoit confirmations via USSD/SMS, consulte son solde mobile money, vote assemblée virtuelle. *JTBD* : accès < 10 km de réseau 2G. (implicite §2)

3. **Gestionnaire union/faîtière** : supervise 5-50 coopératives, 5k-50k producteurs. Tableau de bord mensuel intrants, paiements, conformité. *JTBD* : rapports en 1 jour, pas 1 semaine. (§2.2)

4. **Acheteur/exportateur** : reçoit des lots, génère DDS EUDR, soumet TRACES NT (Commission EU). Chaîne aval. *JTBD* : DDS en 1 clic dès arrivée lot. (§2.3)

5. **Bailleur (Banque Mondiale, AFD, USAID)** : vue M&E sur projectsqu'il finance. Indicateurs LogFrame temps réel auditable. *JTBD* : pas d'Excel, données live. (§2.4)

### 1.2 Les 12 modules fonctionnels et dépendances

| # | Module | Périmètre | MVP | Dépendances |
|---|--------|-----------|-----|-------------|
| 1 | **Registre membres** | KYC, CNIB, parts, ménage, genre, âge | P0 | AUTH (Kratos), Vault PII |
| 2 | **Cartographie parcelles** | Polygones GPS, surface, cultures, rotation | P0 | PostGIS, RLS, CRDT (Automerge) |
| 3 | **Conformité EUDR** | Validation cut-off Hansen/JRC, DDS, TRACES NT submit | P0 | Module 2, validateur geospatial, Vault certs |
| 4 | **Traçabilité récolte** | Lots, sous-lots, balance BLE, QR codes | P1 | Modules 1-2, KAYA sessions, MinIO (photos) |
| 5 | **Gestion intrants** | Catalogue, crédit distribution, remboursement | P1 | Module 1, audit-lib |
| 6 | **Paiements mobile money** | Orange/Moov/Wave, idempotence, réconciliation | P1 | Modules 1,4,5 ; KAYA cache ; Redpanda CDC |
| 7 | **Comptabilité OHADA** | SYSCOHADA, journaux, exports Sage/Tompro | P2 | Modules 1,4,5,6 |
| 8 | **Marketplace acheteurs** | Annonces, contrats, escrow, signature | P2 | Modules 1,4,6 ; PKI Vault |
| 9 | **Reporting bailleurs** | LogFrame, DHIS2, exports BM/AFD/USAID | P2 | Modules 1-8 ; ClickHouse agrégats |
| 10 | **Formation & vulgarisation** | MOOC langues nationales, audio, quiz USSD | P3 | Module 2, terroir-ussd |
| 11 | **Crédit & assurance** | Scoring, index climatique | P3 | Modules 1,5,6 ; partenaires SFD |
| 12 | **Marché du carbone** | MRV, Verra/Gold Standard, tokenisation | P4 | Modules 1,2 ; partenaires carbone |

**Topologie de dépendances** : Module 1 (membres) + Module 2 (parcelles) = base ; Module 3 (EUDR) récolte tout sur Module 2 ; Module 4-6 (récolte+paiement) convergent vers audit-lib + KAYA + CDC. Modules 7-9 dépendent essentiellement de 1-6. Modules 10-12 extensions post-MVP. (PLAN-TERROIR.md §3)

### 1.3 Parcours utilisateurs × entités (swimlane)

```
Agent terrain offline          Backend sync           Union/Exportateur
─────────────────────          ────────────           ────────────────

[Signup sur app]  ───→  [JWT Kratos + local cache]  ───→  [Gestionnaire valide]
       ↓                                                           ↓
[Saisie 50 livraisons]        [Queue Redpanda CDC]            [Dashboard]
[Photos CNIB]           →      [Sync delta partial]      →    [Indicateurs]
[GPS parcelle]                 [Conflict resolve CRDT]         [Exports bailleur]
[Poids + QR]                   [Audit-log append-only]
       ↓                                ↓
[Retour réseau 3G/4G]         [Réconciliation 24h]
[Sync full batch]             [Mobile money dispatch]
                              [DDS génération]
                              [TRACES NT submit]
```

(PLAN-TERROIR.md §2, implicite dans les ADRs)

### 1.4 Roadmap P0 à P6 et jalons Go/No-Go

| Phase | Durée | Livrables clés | Critère succès |
|-------|-------|---|---|
| **P0 Discovery** | 6 sem | 10 interviews, design system, 6 ADRs, port-policy | 3 LOI signées |
| **P1 MVP core** | 12 sem | terroir-core, app agent terrain, validateur EUDR | 500 producteurs, 100 DDS acceptées |
| **P2 Récolte+intrants+paiement** | 10 sem | Modules 4-5-6, Orange Money + Wave, SMS notifs | 50M CFA payés, 1 campagne complète |
| **P3 Marketplace+reporting** | 8 sem | Modules 7-8-9, e-signature, bailleur dashboards | 1 contrat acheteur signé |
| **P4 Scale 5 coops** | 12 sem | Multi-tenant (schema-per-tenant), runbooks, support tier-1 | 10k producteurs, 99.5% SLO |
| **P5 Formation+crédit** | 12 sem | Modules 10-11, partenariat SFD + ARC | 2 produits financiers actifs |
| **P6 Carbone+multi-pays** | 16 sem | Module 12, déploiement CI/SN/ML/BJ | 1er crédit carbone vendu |

**Jalons Go/No-Go** : (PLAN-TERROIR.md §8)
- Fin P1 : pilote exportateur signe contrat SaaS (proof-of-value)
- Fin P2 : ARR 50 k€ (revenue traction)
- Fin P4 : break-even opérationnel coopérative type (unit economics)

---

## SECTION 2 — DÉCISIONS ARCHITECTURALES (ADRs)

### ADR-001 — Framework mobile (React Native + Expo)

**Décision** : RN + Expo bare workflow avec Hermes engine. (ADR-001-mobile-framework.md)

**Justification** : 
- Pool dev uniée (web React + mobile RN) → vélocité 12-sem atteignable
- Expo modules (caméra, GPS, BLE, NFC) couvrent tous capteurs terrain
- EAS Update réduit friction PlayStore en zone bande passante (delta ~2 MB)
- Hermes compatible Android Go (1 GB RAM, cible Tecno Spark)
- TypeScript bout-en-bout (validation Zod, types domaine partagés avec terroir-web-admin)
- Bare workflow = porte de sortie native si besoin module custom

**Alternatives écartées** : Flutter (aucun dev Dart interne, écosystème offline-first moins mature) ; Native Android only (coût x2 pour iOS futur) ; PWA (permissions natives insuffisantes : BLE limité, NFC quasi absent).

**Métriques de succès** : cold start ≤ 3s Tecno Spark Go ; sync 50 livraisons + photos ≤ 2 min EDGE ; APK ≤ 25 MB ; 0 crash sur 100 livraisons synthétiques. (§10, Métriques)

**Conséquences** : risque dépendance Expo (vendor lock-in modéré) ; si module BLE custom requis pour balances exotiques, devra écrire module natif ; performance carte MapBox GL sur Android Go à benchmarker P1. *Mitigation* : bare workflow dès J0, feature flag carte → fallback liste si RAM insuffisant. (§Conséquences, Mitigations)

### ADR-002 — Sync & résolution conflits (Hybrid CRDT/LWW/append-only/ACID)

**Décision** : Option C — Hybrid par type d'entité. (ADR-002-sync-conflict-resolution.md)

Chaque entité utilise la stratégie qui matche sa sémantique :

| Entité | Stratégie | Raison |
|--------|-----------|--------|
| **Livraison récolte** | Append-only event sourcing | Critique audit, jamais de conflit (1 agent = 1 livraison) |
| **Paiement mobile money** | Transaction ACID centralisée | Idem + idempotency key UUID v7 côté client |
| **Distribution intrants** | Append-only event log | Idem |
| **Photos/scans CNIB** | Append-only + S3 immuable | UUID local, pas d'écrasement |
| **Parcelle (polygone GPS)** | CRDT (Automerge) | Modéré concurrent edits, merge safe, perte donnée inacceptable EUDR |
| **Profil producteur** | CRDT (Automerge) | Notes agent, observations terrain |
| **Statuts administratifs** | LWW (server timestamp) | Validé/rejeté/en révision, unilatéral superviseur |
| **Champs simples membres** | LWW (server timestamp) | Téléphone, photo |

**Détails techniques** : (§Détails techniques)
- Idempotency keys : UUID v7 générés client à chaque création
- Conflict log : table `terroir_conflict_log` avec snapshot avant/après → audit + rollback humain
- Photo immuable : UUID local, upload S3 ETag = SHA-256, pas d'écrasement
- Vector clock : chaque agent_id incrémenté à chaque opération offline
- Parcelle conflict detection : si `ST_HausdorffDistance(old, new) > 50m` → flag, alerte gestionnaire union, validation manuelle 24-72h

**Alternatives écartées** : Last-Write-Wins partout (perte silencieuse si 2 agents éditent même polygone) ; CRDT partout (surcharge espace, mauvais fit transactions monétaires) ; Event sourcing global (courbe apprentissage trop forte).

**Conséquences** : Séminatique correcte, auditabilité préservée, aucune perte donnée. *Négatif* : 4 stratégies à maintenir, Automerge ajoute ~200 KB bundle mobile, coût stockage history. *Mitigation* : documenter chaque entité dans schéma, property-based tests parcelles CRDT, compaction Automerge nightly (history → snapshot). (§Conséquences, Mitigations)

**Métriques de succès** : 0 perte donnée sur 1000 sync multi-agents ; latence sync ≤ 50 KB delta typique ; détection conflit polygone < 24h ; compaction conserve < 5x snapshot. (§Métriques de succès)

### ADR-003 — USSD multi-provider avec routage adaptif

**Décision** : Option D — Multi-provider abstrait, routage par pays/opérateur + fallback. (ADR-003-ussd-provider.md)

Architecture (§Architecture) :

```
terroir-ussd (Rust Axum 8834)
  ├─ UssdRouter lookup(country, msisdn) → provider
  ├─ Fallback chain (primary → secondary)
  ├─ Adapters : AfricaTalking, Hub2, Twilio (mock)
  └─ Menu DSL (YAML state machine) → rendu localisé FR/Mooré/Dioula/...
```

**Routage initial proposé** (à confirmer P0) :
- **BF** : Hub2 primary, Africa's Talking fallback, SMS Twilio emergency
- **CI** : Africa's Talking primary, Hub2 fallback
- **SN** : Africa's Talking primary, Hub2 fallback
- **Mali/Bénin/Togo** : Hub2 primary, Africa's Talking fallback

Twilio gardé SMS-only emergency tant qu'on n'a pas dérisqué providers africains. Code court USSD BF : `*144*FASO#` (à négocier ARCEP) ; sinon codes provider. (§Routage par défaut)

**Sécurité** : Aucun secret en config statique → Vault `faso/terroir/ussd/{provider}/{key}` ; rotation 90j tokens API ; validation IP whitelisting webhooks ; HMAC signature callbacks entrants ; rate-limiting 5 sessions/minute/MSISDN. (§Sécurité)

**Coût an 1** (50k producteurs actifs) : USSD ~4.5M FCFA/an (~7k€) ; SMS ~150M FCFA/an (~230k€) *gros poste* à refacturer pricing exportateur. (§Coût estimé)

**Conséquences** : Redondance + négociation coûts ; Twilio retirable dès dérisquage. *Négatif* : complexité intégration (~3-4 sem dev, 3 adapters) ; tests E2E nécessite mocks fidèles. (§Conséquences)

**Métriques de succès** : USSD session success rate ≥ 95% ; latency p95 ≤ 2s/interaction ; SMS delivery ≥ 98% ; 0 secret en config. (§Métriques de succès)

### ADR-004 — Schéma EUDR DDS et versioning

**Décision** : Option C — Modèle interne stable + adaptateurs versionnés vers DDS UE. (ADR-004-eudr-dds-schema.md)

**Modèle interne** (immuable) : `TerroirDdsContext` avec operator, product, plots[], upstream_dds_refs[], risk_assessment, collected_at. Découple TERROIR du schéma UE qui évolue (v1.4 → v1.5 attendue mi-2026).

**Mappers versionnés** : Crate Rust `terroir-eudr-dds-mapping` avec fichiers `mapping/v1_4.rs`, `mapping/v1_5.rs`, tests snapshots + corpus de référence. Sélection runtime via env var `TERROIR_DDS_SCHEMA_VERSION` (défaut latest stable). (§Mapper versionné)

**Stockage** : (§Stockage)
1. PostgreSQL table `eudr_dds` : metadata, statut, ref TRACES NT, version schéma
2. MinIO objet immuable S3 object-lock 5 ans : payload JSON intégral signé Ed25519
3. Hash SHA-256 stocké en PG → audit non-tampering

**Soumission TRACES NT** : Service `terroir-eudr` worker async, retry exponentiel 5/15/60/240 min, idempotency key = UUID v7, statut `draft → validating → submitted → accepted/rejected`. Webhooks TRACES NT ou polling /status. (§Soumission TRACES NT)

**Validation locale** : JSON Schema (copié UE), validation geom (surface ≥ seuil, fermé, pas auto-intersection), validation cut-off déforestation, validation HS code whitelist. (§Validation locale)

**Compatibilité** : Si UE casse v1.4 → v1.5, ajouter nouveau mapper, migrer backfill batch lots non soumis. DDS soumises restent version d'origine (immutable object-lock). (§Compatibilité ascendante)

**Conséquences** : Indépendance schéma UE (encapsulation) ; audit rejouabilité ; migration simple. *Négatif* : mapper à maintenir (~1-2 sem/release UE). *Mitigation* : veille proactive newsletter EU, corpus partagé partenaires UEMOA. (§Conséquences)

**Métriques** : DDS submission success ≥ 99.5% / 30j ; 0 lot bloqué douane ; latence p95 e2e ≤ 24h ; migration v1.4 → v1.5 < 1 sprint sans interruption. (§Métriques de succès)

### ADR-005 — Chiffrement PII (pgcrypto + Vault Transit envelope)

**Décision** : Combinaison D+F — Envelope encryption hybride par classe donnée. (ADR-005-pii-encryption.md)

**Classification** : (§Classification)
- **Quasi-fort** (CNIB num, MSISDN, GPS domicile) → pgcrypto + Vault Transit envelope PostgreSQL colonne
- **Fort** (biométrie template, photo CNIB) → AES-256-GCM appli + DEK chiffrée Vault Transit ; MinIO + clé en PG
- **Public-après-anonymisation** (stats agrégées, géoloc parcelle) → aucun chiffrement

**Schéma envelope** (§Schéma envelope) :
```
Vault Transit (terroir-pii-master, rotation 90j, conservation 5 versions)
  ↓ DEK encrypt/decrypt
Application (AES-256-GCM, DEK in-memory only, DEK_chiffré en PG)
  ↓
PostgreSQL (cnib_encrypted bytea, msisdn_encrypted bytea, pii_dek bytea, pii_kek_version)
```

**Photos/scans volumineux** : Chiffrement appli AES-256-GCM avant upload S3 ; DEK par photo chiffré en PG ; MinIO object-lock 5 ans + strict access policies. (§Photos / scans)

**Rotation** : Vault Transit auto-rotate 90j ; re-chiffrement DEK à la lecture si version désuète (lazy migration) ; job nightly forcé si version dépréciée 30j. (§Rotation)

**Recherche/index** : Champs chiffrés non-indexables directement. Pour MSISDN : colonne `msisdn_hash` (HMAC-SHA256 sel partagé Vault) → recherche exacte. Pour CNIB : aucune recherche directe, accès toujours par member_id. (§Recherche / index)

**Suppression/RTBF** : Effacement DEK (cryptoshredding) + tombstone PG. Données chiffrées restent irrécupérables sans DEK. Métadonnées audit conservées 5 ans. (§Suppression / droit à l'oubli)

**Conformité** : Loi 010-2004 BF + RGPD UE + Convention Malabo. DPIA à rédiger avant P1. Consentement audio horodaté MinIO. DPO désigné. Procédure RTBF SLA 30j max. Notification breach < 72h. (§Conformité)

**Conséquences** : Conformité légale, compromise DBA seul ≠ exposition, rotation transparente, cryptoshredding rapide. *Négatif* : latence read +5-10ms (call Vault unwrap DEK), code appli complexe, cache PII en clair interdit. *Mitigation* : helpers Rust + macro `#[encrypted]`, cache DEK 60s mémoire, bench latence P1 (cible p95 read ≤ 100ms). (§Conséquences)

**Métriques** : 0 PII en clair logs (audit mensuel) ; latence read p95 ≤ 100ms ; rotation 90j sans interruption ; 0 incident sécurité PII an 1. (§Métriques)

### ADR-006 — Multi-tenancy (schema-per-tenant + Keto ABAC + RLS)

**Décision** : Option D — Hybride : schema-per-tenant pour métier, RLS pour sous-tenants. (ADR-006-multi-tenancy.md)

**Définition tenant** : (§Définition « tenant »)
- **Tenant primaire** = coopérative cliente (contrat SaaS signé)
  - 1 schema PG par tenant : `terroir_t_<slug>` (ex: `terroir_t_uph_hounde`)
  - 1 bucket MinIO logique : `terroir-t-<slug>`
  - 1 namespace Keto pour permissions
- **Union** = méta-tenant regroupant N coops
  - Pas de schema dédié (données = vues croisant schemas membres)
  - Permissions Keto : union_admin a `read` sur tous schemas membres
- **Exportateur** = consommateur cross-tenant
  - Aucun schema propre, accès via Keto policies (lecture restreinte contrats actifs)
- **Bailleur** = consommateur M&E
  - Vues agrégées schema `terroir_donor_<slug>` alimentées CDC Redpanda
  - Pas d'accès PII brutes (uniquement agrégats anonymisés)

**Architecture** (§Architecture) :
```
ARMAGEDDON (front gateway)
  ├─ JWT validation + claim extraction (tenant_id, role, allowed_schemas[])
  ↓
terroir-core / terroir-eudr / etc.
  ├─ SET search_path TO terroir_t_<slug>;
  ├─ Connection pool tenant-scoped pgbouncer
  ├─ Keto check cross-tenant queries
  ↓
PostgreSQL
  ├─ schema terroir_shared (catalogue, references)
  ├─ schema terroir_t_<slug> (par coop) ← isolation SQL
  ├─ schema terroir_union_<u> (vue union)
  └─ schema terroir_donor_<d> (vue M&E)
```

**Migrations** : Flyway multi-schema mode. Workflow : `flyway migrate -schemas=terroir_t_<slug>` lancé CI. Test : Flyway sur tenant test + snapshot avant prod. Rollback par tenant individuel. (§Migrations)

**Onboarding tenant** : (§Onboarding tenant)
1. Création tenant terroir-admin (port 9904)
2. Migration Flyway sur nouveau schema
3. Bucket MinIO + policies
4. Namespace Keto + relations init
5. JWT claim `tenant_id` injecté Kratos session
6. SLA : ≤ 5 min clic → coop opérationnelle

**Cross-tenant queries** (exportateur, bailleur) : Vue matérialisée `terroir_shared.export_dds_v` rafraîchie CDC. Filtrage Keto + tenant_id whitelist par utilisateur. Audit log every read (Loki) détection anormal. (§Cross-tenant queries)

**Sécurité** : (§Sécurité)
- `SET search_path` serveur seulement, jamais client
- pgbouncer rejette requêtes manipulant search_path en client
- Audit logs cross-schema → revue mensuelle RSSI
- Tests pen tenant : tentative cross-tenant régulière suite Playwright
- RLS activé tables transverses critiques (defense in depth)

**Volume cible an 3** : ~50 coops clients (50 « top tenants »), 5-10 exportateurs, 3-5 bailleurs, 200k producteurs cumulés, 50 GB/an PG, 500 GB/an MinIO. (PLAN-TERROIR.md §6, ADR-006 §Contexte)

**Conséquences** : Isolation forte (SQL level), restore par tenant trivial, audit simplifié, migration différentielle possible. *Négatif* : N migrations à orchestrer (CI nécessaire), pool connexions complexe (pgbouncer mode transaction + search_path), backup volumes scaling. *Mitigation* : tool provisioning auto (terroir-admin), monitor drift (alarme si tenant > 7j retard migration), backup différentiel + retention adaptée. (§Conséquences, Mitigations)

**Coût** : PG un seul cluster (HA replica + standby), partitionnement sain → 30-50% économie vs DB-per-tenant. (§Coût)

**Métriques** : 0 leak cross-tenant (test pen mensuel) ; provisioning ≤ 5 min p95 ; migration ≤ 30 min total 50 tenants ; p95 query ≤ 100ms. (§Métriques de succès)

---

## SECTION 3 — EUDR VALIDATOR SPIKE (geospatial validation)

**Scope** : Design (pas de code) du module qui ingère polygone GPS + date récolte, valide déforestation post-2020, génère risque score + evidence preuves. (eudr-validator-spike.md)

### 3.1 Sources données (Hansen GFC + JRC TMF)

**Hansen Global Forest Change (UMD/GLAD)** : (§2.1)
- Producteur : University of Maryland, Hansen et al.
- Résolution 30 m (Landsat), couverture 2000-2024, mises à jour annuelles
- Format GeoTIFF tuiles 10°×10°, bande `lossyear` (année perte, 1-24 = 2001-2024)
- Taille totale ~50 GB compressé (monde)
- Licence : non-commercial libre, commercial avec attribution

**JRC TMF (Tropical Moist Forests — Joint Research Centre Commission EU)** : (§2.2)
- Producteur : Commission UE
- Résolution 30 m, spécifique forêts tropicales humides (zone agricole UEMOA pertinente)
- Bandes : `transition` (12 classes), `deforestation_year`
- Licence ouverte PSI 2019
- **Recommandé officiellement par DG ENV comme dataset autoritaire EUDR**

**Stratégie** (§2.3) : Utiliser les deux avec règle de priorité :
1. Si JRC TMF couvre la zone → autoritaire pour conformité
2. Sinon Hansen GFC → preuve auxiliaire
3. Si désaccord → flag manuel + revue agronome

| Critère | Hansen GFC | JRC TMF |
|---------|-----------|---------|
| Reconnaissance UE | Forte | **Officielle** |
| Couverture sahel/savane | Bonne | Limitée (TMF = forêts humides) |
| Mise à jour | Annuelle | Annuelle |
| Précision sahel/savane | Moyenne | Faible (non couvertes) |

**Sources complémentaires P2+** : Planet Labs 3m/journalier (partenariat ESA) ; Sentinel-2 Copernicus 10m/5j vérification ad hoc ; NASA SERVIR sahel-spécifique. (§2.4)

### 3.2 Architecture du validateur

**Crate Rust** : `terroir-eudr-validator` (workspace `INFRA/terroir/`) avec composants (§3.1) :

```
terroir-eudr-validator
  ├─ public API: validate(polygon, harvest_date) → Outcome
  ├─ TileFetcher
  │   ├─ Cache local LRU disk (5 GB)
  │   ├─ Fallback HTTP S3 signed URL UMD/JRC mirror
  │   └─ Vérification hash SHA-256
  ├─ HansenAdapter (read lossyear)
  ├─ JrcTmfAdapter (read transition/deforestation_year)
  └─ Reasoner
      ├─ cut-off check 2020-12-31
      ├─ polygon clip + sample
      ├─ majority rule + agreement sources
      └─ risk_level heuristic
```

**Outcome** : `{ deforestation_post_2020: bool, score: f64 (0.0-1.0), sources: [HANSEN|JRC_TMF], evidence: {tile_urls, dataset_versions, computed_at}, risk_level: Low|Medium|High }` (§1)

**Crates Rust** (à valider) : `gdal-rs` ou `geozero` + `geo` (GeoTIFF, géométrie) ; `proj` (reprojection) ; `serde` (DDS payload) ; `jsonschema` (validation UE) ; `reqwest` + `tokio` (fetch) ; `lru-disk-cache` (cache) ; `sha2` (intégrité). (§3.2)

### 3.3 Algorithme (pseudo-code)

```
fn validate(polygon, harvest_date):
    cut_off = 2020-12-31
    
    # Bounding box
    bbox = polygon.bounds()
    
    # Tuiles à charger
    tiles_hansen = TileFetcher.tiles_for_bbox(bbox, HANSEN)
    tiles_jrc = TileFetcher.tiles_for_bbox(bbox, JRC_TMF)
    
    # Pour chaque pixel intersectant polygone :
    for pixel in polygon.intersect(tiles_hansen):
        if pixel.lossyear > 20:  # perte après 2020
            mark_deforested(pixel, HANSEN)
    
    for pixel in polygon.intersect(tiles_jrc):
        if pixel.deforestation_year > 2020:
            mark_deforested(pixel, JRC_TMF)
    
    # Score = surface défrichée / surface totale
    score = sum(deforested_area) / polygon.area
    
    # Décision
    if jrc_tmf_covers(polygon):
        deforestation_post_2020 = (jrc_score > 0.0)
    else:
        deforestation_post_2020 = (hansen_score > 0.05)  # tolérance 5% bruit
    
    # Risk level heuristique
    risk_level = match score:
        0.0..=0.01  → Low
        0.01..=0.10 → Medium
        _           → High
    
    return Outcome { deforestation_post_2020, score, sources, evidence, risk_level }
```

(§3.3)

### 3.4 Cas limites

| Cas | Stratégie |
|-----|-----------|
| Polygone chevauchant 2 tuiles | Fusion seamless via `geo::ops::union` |
| Polygone < 1 pixel | Erreur `PolygonTooSmall`, exiger surface ≥ 0.0009 km² |
| Hansen vs JRC désaccord | Flag `Disagreement`, revue manuelle, conservé evidence |
| Datasets mis à jour pendant batch | Hash SHA-256 figé début campagne, alerte si rotation |
| Polygone hors zone tropical (sahel sec) | JRC TMF retourne « no data » → fallback Hansen, flag `coverage: hansen-only` |
| Auto-intersection ou trou | `InvalidGeometry` rejeté upstream message clair |
| Date récolte incohérente (futur) | `InvalidHarvestDate`, refus immédiat |

(§3.4)

### 3.5 Plan validation & KPI

**Corpus de test P0** : (§4.1)
- 100 parcelles BF coton (validées terrain agronome)
- 50 parcelles CI cajou (déforestation post-2020 connue)
- 50 parcelles SN sésame
- 20 cas adversariaux (< 1 ha, chevauchant frontière, multi-polygones)
- Snapshots DDS attendus → tests `insta` + corpus golden

**KPI** : (§4.2)
- Latence p95 ≤ 300 ms parcelle (cache chaud)
- Latence p95 ≤ 5 s (cache froid, tuile à fetch)
- Précision agronome ground truth ≥ 95% corpus référence
- 0 faux négatif cas connus déforestation post-2020

**Bench** : (§4.3)
- 10k parcelles batch → < 10 minutes (cache chaud)
- Mémoire ≤ 1 GB worker
- Concurrent safety : 100 validations parallèles, 0 corruption cache

### 3.6 Stratégie cache (3 niveaux)

| Niveau | Détail |
|--------|--------|
| **L1 mémoire** | LRU 100 tuiles décodées (mmap interior mut) → instant |
| **L2 disque** | Tuiles GeoTIFF brutes, 5 GB max, eviction LRU |
| **L3 réseau** | MinIO souverain + fallback UMD/JRC originaux |

**Pré-chargement** : Job nightly pré-charge tuiles zones coopératives actives. Stats cache hit Prometheus. **Invalidation** : Datasets annuels (Hansen v1.x, JRC TMF v1.x) → version explicite path (`hansen/v1.11/lossyear/N00E000.tif`) ; job rotation nouvelle version → re-validation différentielle → alerte si re-classification. (§5)

### 3.7 Sécurité & audit

- **Intégrité** : Hash SHA-256 chaque tuile (catalogue signé Ed25519 publié UMD/JRC ou TERROIR)
- **Audit** : Chaque Outcome immuable S3 object-lock 5 ans, evidence URLs immuables → rejouabilité en litige
- **Pas de secrets en clair** : Token MinIO → Vault `faso/terroir/eudr/minio-key`
- **Logs** : Aucun PII, coordonnées GPS que strictement nécessaire

(§6)

---

## SECTION 4 — COMPATIBILITÉ INFRA FASO

### 4.1 Réutilisations confirmées (modules existants)

**auth-ms (ORY Kratos + Keto)** — Authentification identité producteurs/coops/exportateurs/bailleurs. TERROIR s'y connecte directement. (port-policy.yaml §allocations auth-ms 8801, kratos-public 4433, keto-read 4466)

- **Signification** : terroir-core + autres services upstream Kratos session via JWT
- **Pattern existant** : auth-ms expose `/session` (Kratos hook), `/jwks` (clé publique), gRPC AuthGrpcService pour service-to-service
- **Intégration TERROIR** : terroir-mobile-bff valide JWT Kratos avant API agent terrain ; terroir-core gère RBAC via Keto (voir ci-dessous)

**Keto ABAC (autorisation)** — Namespace actuellement : User, Role, Platform, Resource, Department, AdminRole (phase 4.b), Capability (2026-04-30 amendment). (ory/keto/config/namespaces.ts)

- **Pour TERROIR** : extension namespaces avec `Cooperative`, `Parcel`, `DdsSubmission`, `MobileMoneyTransaction` ?
- **Architecture** : ADR-006 mandate Keto ABAC par tenant + cross-tenant, mais **Keto actuellement utilisé par FASO en RBAC majoritaire** (User → Role → Platform/Resource avec permissions hierarch)
- **Question clé** : Keto ABAC mature ? Ou TERROIR doit-il ajouter de la logique appli (bearer token + claim validation) ?

**KAYA (cache + sessions + queues)** — In-memory DB redis-like, RESP3, Pub/Sub. (kaya:6380)

- **Usages TERROIR** :
  - Session offline agent terrain : `terroir:session:{agent_id}:{session_token}`
  - Idempotency cache mobile money : `terroir:idempotent:{request_uuid}` TTL 24h
  - Rate-limit counter USSD : `terroir:ratelimit:{msisdn}:session_count` TTL 60s
  - Pub/Sub topics : `terroir.member.*`, `terroir.harvest.*`, `terroir.payment.*` (CDC triggers)
  - Lock distribué : parcelle en validation conflit CRDT

- **Existant** : KAYA déjà utilisé auth-ms (JWT blacklist, session limit), poulets-api (idempotency, cache). TERROIR réutilise même pattern, namespacing via prefixes `terroir:*`

**ARMAGEDDON gateway** — Proxy Pingora (frontend, mTLS, rate-limit, ext_authz). (port 8080)

- **Pour TERROIR** : Tous services terroir derrière ARMAGEDDON comme route `/api/terroir/*` (terroir-core, terroir-eudr, terroir-payment, terroir-mobile-bff, terroir-buyer)
- **ext_authz Keto** : ARMAGEDDON consulte Keto avant de router requête vers backend
- **Pattern existant** : Déjà utilisé pour auth-ms, poulets-api, bff-nextjs

**Redpanda (event bus Kafka)** — Message broker souverain, Topics existants : `auth.*`, `poulets.*` CDC. (redpanda:19092)

- **Pour TERROIR** : Topics `terroir.member.*`, `terroir.harvest.*`, `terroir.payment.*` pour CDC (Change Data Capture) PG → dashboards bailleur, analytics ClickHouse
- **Pattern** : Outbox pattern existant (audit-lib), appliqué à parcelles CRDT sync

**audit-lib (append-only immutable log)** — Spring Boot shared library, JPA aspects, `@Audited` annotation. (shared/audit-lib/)

- **Pour TERROIR** : Audit immuable livraisons récolte, paiements mobile money, DDS soumissions TRACES NT
- **Extensiob** : TERROIR ajoute table partitioning par tenant (cf. ADR-006) ?

**notifier-ms (SMS/email)** — Kafka consumer, templates Handlebars, Mailpit en dev. (port 8803)

- **Pour TERROIR** : Notifications producteur USSD result, notifications union payment confirmation, notifications bailleur DDS status
- **Pattern** : Channel existant (email/SMS) pour TERROIR, routing via `terroir.event.*` topics

**Vault (secrets + PKI)** — HashiCorp Consul + Vault. (port 8200, 8500)

- **Pour TERROIR** : 
  - KV `faso/terroir/<service>/<usage>` (secrets DB credentials, API keys USSD providers, S3 creds)
  - Transit `terroir-pii-master` (clé rotation 90j envelope encryption)
  - PKI client certs TRACES NT mTLS
  - Policies read-only par service (principle least privilege)

**MinIO / S3 storage** — Object store souverain (photos, documents, archivage 5 ans EUDR).

- **Pour TERROIR** : Buckets logiques par tenant `terroir-t-<slug>` ; object-lock 5 ans DDS soumises ; photos CNIB, scans EUDR justificatifs

**ClickHouse (analytics)** — OLAP engine souverain. (Implicite plan, pas encore en compose)

- **Pour TERROIR** : Reporting bailleurs agrégats (M&E LogFrame, indicateurs), queries analytiques « combien producteurs par région? ». CDC Redpanda alimente ClickHouse.

### 4.2 Nouveautés à introduire

**Schema-per-tenant Postgres** — Nouveau pattern pour FASO. (ADR-006)

- **Impact** : Flyway multi-schema orchestration, connection pool pgbouncer mode transaction + search_path, backup/restore par tenant
- **Architecture existante** : PG actuellement single-tenant (auth_ms DB, poulets_db)
- **Risque** : Compexité opérationnelle migrations, monitoring 50 tenants en drift

**Keto ABAC extensible** — FASO actuel : RBAC basique (User → Role → Platform). TERROIR demande ABAC granulaire (tenant-specific resource attributes).

- **Question** : Keto supporte-t-il ABAC contexte-aware (tenant_id in JWT claim) ? Ou faut-il custom logic applicatif ?
- **Décision nécessaire P0** : Étendre OPL Keto namespaces vs. JWT claim validation côté service ?

**RLS PostgreSQL (Row-Level Security)** — Jamais utilisé FASO avant. (ADR-006)

- **Pour TERROIR** : Defense-in-depth cross-tenant isolation (backup if search_path ou pgbouncer fail)
- **Risk** : RLS peut être contourné superuser ; audit complexe (qui a modifié quelle ligne ?)

**Vault Transit envelope encryption** — Vault existant pour KV, pas encore Transit. (ADR-005)

- **Nouveau** : terroir-pii-master clé rotation, DEK enveloppe, AES-256-GCM applicatif côté Rust
- **Dépendance** : Rust appli doit intégrer vaultrs SDK, appels Vault à chaque read PII
- **Latence** : +5-10ms par requête (Vault unwrap DEK) → bench obligatoire

**CRDT (Automerge/Yjs)** — Jamais utilisé FASO. (ADR-002 parcelles)

- **Impact** : Bundle mobile +200 KB, storage overhead history, compaction nightly nécessaire
- **Expertise** : Team Rust/JS doit s'approprier CRDT (learning curve 2-3 sem)

**Multi-provider USSD Hub2 + Africa's Talking** — Twilio actuel pour SMS urgences. (ADR-003)

- **Nouveau** : Hub2 et Africa's Talking pour USSD primary, routage adaptatif par pays/opérateur
- **Integration** : 3-4 sem dev (3 adapters + DSL state machine USSD)
- **Vendor management** : 3 contrats vs 1 actuellement

**Mobile money Orange/Moov/Wave/MTN intégration** — Patterns existants poulets-platform ? (ADR-006 modules 6)

- **Question clé** : terroir-payment Java Spring réutilise-t-il les patterns mobile money existants poulets-api, ou from scratch ?
- **Vérifier** : Y a-t-il SDK/adapter poulets-platform pour Orange Money / Wave ?

**Mobile app RN+Expo** — Premier app native FASO. (ADR-001)

- **Nouveau** : EAS Update OTA distribution, SigningCertificate gestion, PlayStore submission workflows
- **DevOps** : CI/CD Expo build + sign + deploy (GitHub Actions ou custom?)

---

### 4.3 Conflits & risques souveraineté

**Twilio cloud-foreign** — US-based SaaS, oppose CLAUDE.md §3 (souveraineté). Conservé en SMS-only fallback ADR-003, avec plan de retrait dès Hub2/Africa's Talking mature. **Risque** : Si Hub2/AT downtime, fallback Twilio expose donnée USA. **Mitigation** : Audit hebdomadaire Vault `faso/terroir/ussd/twilio/*` usage ; plan retrait Twilio documenté ; alternative SMS-only : SMS directement opérateur BF (nécessite négociation ARCEP, hors-scope P1).

**Hub2 / Africa's Talking SaaS** — Non-EU providers (Kenya-based). Techniquement pas mentionnés CLAUDE.md comme violation (exception HashiCorp/tiers justifiée). **Question** : Y a-t-il clause « données restent Afrique » avec Hub2/AT contrats ? DGFC/ANATEL BF a-t-il exigences ?

**Hansen GFC + JRC TMF données sources externes** — USGS (USA) + EU JRC. **Licence** : Hansen non-commercial libre, commercial attribution. JRC PSI 2019 ouverte. **Risque** : Service downtime UMD/JRC → validateur EUDR bloqué. **Mitigation** : Cache local + PreCache nightly zones coops actives + fallback MinIO mirror (respect licenses). Version figée datasets (v1.11) → immutable, audit trail.

**Loi 010-2004 BF vs. RGPD UE** — PII producteurs africains, mais si acheteur UE accède données, RGPD s'applique. **Mitigation** : DPIA avant P1, transferts encadrés clauses contrats, consentement éclairé audio, DPO externe/mutualisé.

---

### 4.4 Synergies architecturales positives

**Kratos + Keto déjà en place** → TERROIR n'invente pas authn/authz, hérite patterns matures.

**KAYA + Redpanda + CDC** → Architecture eventsourcing naturelle livraisons + paiements.

**Vault + SPIRE déjà opérés** → Service mesh mTLS + secrets management mature.

**audit-lib shared** → Compliance audit décentralisé par module vs. monolithic.

**Observabilité Prometheus + Loki + Tempo** → TERROIR omet jamais PII logs, bénéficie d'infra traces distribuées.

**PostgreSQL 17 + PostGIS** → Géospatial native (parcelles polygones), pas besoin service externe géospacetil.

---

## SECTION 5 — GAPS & ZONES GRISES

Listes des points **non clairs** dans les docs, nécessaires avant rédiger plan implémentation. Chaque point = question précise à clarifier.

### 5.1 Datasets CRDT vs. LWW vs. Append-only — Exhaustivité

**Zone grise** : ADR-002 donne une table de stratégies par type entité, mais liste de **toutes les tables** TERROIR manque. Ex: table `cooperative`, `union`, `exporter`, `warehouse_stock`, `contract_terms` — quelle stratégie exactement ?

**Question P0** :
- Lister exhaustivement chaque table/entité TERROIR (modules 1-6) avec sa stratégie de sync
- Pour chaque table potentiellement éditable offline, spécifier : CRDT ? LWW ? Append-only ? Pourquoi ?
- Cas spécial : `cooperative` (tenant), est-elle éditable offline agent terrain ou admin-only ?

### 5.2 Authentification offline agent terrain (degraded mode)

**Zone grise** : ADR-005 décrit chiffrement PII, mais flux authen agent offline flou.

**Question P0** :
- Agent terrain démarre app, réseau down (2G coupé). Session Kratos → JWT ? Stocké où ? `HKDF(password + device_id)` local ?
- JWT offline a combien de temps de validité avant expiration ? (Récit : agent rentre après 14j, session expirée, pas accès → quoi faire ?)
- Biométrie optionnelle agent : si oui, FaceTec enroll online + vérification offline ? Quels fallback offline ?
- Sync au retour réseau : quel endpoint agrège 1000 opérations offline ? Rate-limit ?

### 5.3 Flows USSD et OTP / SMS auth

**Zone grise** : ADR-003 décrit USSD menus, mais OTP pour signup producteur manque.

**Question P0** :
- Producteur signup via USSD : OTP requis ? Si oui, lequel des 3 providers (Hub2/AT/Twilio) l'envoie ? Quel timeout OTP (5 min) ?
- Signup USSD = pas de mot de passe ? Just phone + OTP → anonymous session ?
- Producteur peut-il changer numéro MSISDN ? Faut-il re-enroll OTP ?

### 5.4 Schema-per-tenant — Sizing & limites

**Zone grise** : ADR-006 cible 50 coops. Contraintes réelles ?

**Question P0** :
- **Limite théorique** : PG supporte combien de schemas ? Aucune limite PG spec, mais en pratique (monitoring, backup, migration time) ?
- **Onboarding SLA** : ≤ 5 min création tenant — incluant Flyway migration ? Si oui, schéma de base combien de tables ? Taille migrations v1 ?
- **Backup per-tenant** : pg_dump chaque schema individuellement OK ? Ou un seul dump complet + split batch ?
- **Connection pool pgbouncer** : Combien connexions par tenant ? Default PG max_connections = 100, partager 50 tenants ?

### 5.5 Multi-tenancy + audit — Granularité par-tenant

**Zone grise** : audit-lib append-only existant, mais scoped comment ?

**Question P0** :
- Audit log : global `audit_log` table + `tenant_id` colonne ? Ou `audit_log_t_<slug>` schema par tenant (isolation parfaite) ?
- Loi 010-2004 BF EUDR compliance : conformité audit exigée par-tenant ou globale ?
- Si global : requête audit cross-tenant acceptable juridiquement ? Ou chaque audit query doit rester dans son tenant ?
- Retention audit : 5 ans = 5 ans après suppression donnée ou fixed date ?

### 5.6 Vault Transit envelope — Clés, rotation, compliance

**Zone grise** : ADR-005 décrit architecture, mais détails opérationnels incomplets.

**Question P0** :
- `terroir-pii-master` clé Vault Transit : rotation 90j automatique Vault ou manuel ? Qui approuve (DPO, RSSI, business ?) ?
- KEK (master) vs. DEK (per-record) : taille DEK ? AES-256-GCM ou AES-128-GCM ?
- FASO actuel : Vault Transit déjà configuré ou nouveau ? Si nouveau, how Vault seeded (manual key provision vs. auto-generate) ?
- Quelle KMS alternative si Vault unavailable ? OpenBao ? Tink local KMS ?

### 5.7 Mobile RN+Expo — OTA, signature, distribution

**Zone grise** : ADR-001 mentionne EAS Update, mais détails déploiement vagues.

**Question P0** :
- EAS Update : Expo public cloud ou self-hosted ? (Souveraineté exige self-hosted ?) Cost comparison ?
- Signing certificate APK : FASO-owned ou Expo-owned ? Where stored (Vault? Github Secrets?) ?
- PlayStore submission : qui s'occupe (FASO DBA? External?) ? SLA mise en production APK nouvelle version ?
- Rollback : si nouvelle APK buggy, how long to rollback ? EAS Update instant vs. PlayStore delisted slow ?

### 5.8 Sync conflict resolution — Arbitrage manuel

**Zone grise** : ADR-002 flag conflits CRDT parcelle > 50m distance → revue manuelle. Workflow exact ?

**Question P0** :
- Parcelle conflictée : bloquée pour DDS generation ? Ou autre status interim ?
- Revue manuelle par qui : gestionnaire union 24h, ou agronome expert EUDR qui peut prendre 72h ?
- Tool UI : back-office pour superviser conflits ? Ou alertes email vers gestionnaire + manual Keto permission grant si résolution correcte ?
- Escalation : si 2 agents toujours en désaccord après revue, qu'est-ce qui break deadlock ?

### 5.9 Multi-provider USSD — Stratégie de failover

**Zone grise** : ADR-003 mentionne auto-failover, mais logic manque.

**Question P0** :
- Fallback trigger : après combien d'échecs Hub2 ? 3 rapid fails → switch Africa's Talking ?
- Fallback timing : immediate ou 30s backoff ? (Ne pas DOS fallback provider)
- KAYA flag dynamic : est-ce un redis key `terroir:ussd:provider_override:BF` ? Ou hardcoded route logic ?
- Healthcheck fréquence : KAYA probe chaque 5 min ? Ou sur erreur seulement ?

### 5.10 DDS soumission TRACES NT — Signature & workflow

**Zone grise** : ADR-004 mentionne soumission async + retry, mais logique signature et révocation manque.

**Question P0** :
- DDS générée : signée par qui ? Clé privée exportateur (EORI certif) stockée Vault ? Ou signature centralisée TERROIR ?
- Rejet TRACES NT : qui corrige (exportateur app? Union supervisor?) ? Workflow d'appel vs. re-submit ?
- Annulation DDS : possible après submit ? Si oui, TRACES NT requirement pour notifier annulation ?
- Audit trail : chaque soumission, chaque rejet, chaque appel loggé append-only ? Qui a accès audit log (juste exportateur ou union aussi ?)

### 5.11 Mobile money providers BF — Réutilisation poulets-platform ?

**Zone grise** : terroir-payment Java Spring conçu from scratch ou refactor poulets-api payment ?

**Question P0** :
- Existe-t-il SDK Orange Money / Wave / Moov / MTN integrations dans poulets-platform ? Si oui, réutilisable terroir-payment ?
- Mobile money provider credentials (API key, merchant ID) : stockés Vault global `faso/mobile-money/*` ou Vault par-tenant `faso/terroir/t_<slug>/mobile-money/*` ?
- Reconciliation : combien de fois/jour? Nightly batch ou real-time via provider webhook ?

### 5.12 Agent terrain session offline — Expiration & revocation

**Zone grise** : Session JWT stockée locally app, mais revocation (agent quitte coop) comment ?

**Question P0** :
- Agent renvoyé coop : JWT offline valid 14 jours. Mais KAYA session flag seriez-il avant expiration (revocation immédiate) ?
- Revocation signal : gestionnaire union marque agent inactive → Kratos/KAYA update → app prochaine sync (≤ 14j) check revocation ?
- Risk : rogue agent offline 14j avec stolen JWT ? Defense in depth (biométrie re-check ?) ou accept risk ?

### 5.13 Tenant onboarding — Automation vs. manual

**Zone grise** : ADR-006 SLA ≤ 5 min création tenant, mais auto-vs-manual ambig.

**Question P0** :
- UI : gestionnaire TERROIR (ou co op herself?) clique « Create tenant » dans terroir-admin (port 9904) ?
- Automation : Flyway auto-run ≤ 5 min ? Ou alerter operator « run `flyway migrate -schema=terroir_t_coopname` »?
- Abort scenario : si création échoue (disque plein, quota dépassé), rollback ? Ou manual cleanup Flyway + schema drop?
- Audit : qui a créé tenant, quand, pour quelle coopérative ? Trace dans Vault OU audit-lib?

### 5.14 Buyer portal DDS download — Authentication

**Zone grise** : terroir-buyer portal (port 8835) — quelle auth pour acheteur non-coop ?

**Question P0** :
- Acheteur externe : signup email ? Ou invitation-only par exportateur ?
- DDS PDF download : signed + timestamped (non-repudiation) ? Via Vault PKI certif ou simple HMAC token ?
- Access control : exporter A ne voit DDS coop B ? Keto polices déjà en place ou custom appli logic ?

### 5.15 ARMAGEDDON routing — Centralisé vs. mesh interne

**Zone grise** : ARMAGEDDON front-facing pour terroir-* services, mais intra-service comm comment ?

**Question P0** :
- terroir-core appelle terroir-eudr pour validation : via ARMAGEDDON (public route) ou direct gRPC (internal mesh) ?
- Si direct gRPC : SPIRE mTLS déjà en place ? Sinon, add cert management complexity ?
- Si all via ARMAGEDDON : rate-limiting + auth overhead inter-service ?

### 5.16 EUDR cut-off date et backlog — Historique déjà en place

**Zone grise** : Parcelles enregistrées avant EUDR (avant 2021) — validées ou exemptées ?

**Question P0** :
- **Scenario** : Coopérative a 500 parcelles depuis 2019. Nova EUDR juillet 2026. Doit-on valider rétroactivement post-2020 ?
- **Spec** : cut-off date en code est hard-coded 2020-12-31, mais producteur peut-il fournir preuve parcelle déjà cultivée pré-2020 ?
- **Workflow** : Si parcelle post-2020 en zone déforestée, qui résout (agronome, exportateur, autorité BF) ?

---

## SECTION 6 — PRÉ-REQUIS TECHNIQUES & STACK EXTENSIONS

### 6.1 Dépendances Rust à ajouter

**Core services** (terroir-core, terroir-eudr, terroir-mobile-bff, terroir-ussd, terroir-buyer) :
- **Web framework** : `axum` (async), `hyper` (HTTP/2), `tokio` (runtime)
- **Database** : `sqlx` (async SQL + compile-time check), `sea-orm` (ORM), `sea-query` (query builder)
- **Geospatial** : `geo-types`, `geojson`, `gdal-rs` ou `geozero` + `proj` (reprojections)
- **Serialization** : `serde`, `serde_json`, `serde_avro` (Redpanda schema)
- **Caching** : `redis` + `tokio-redis` (KAYA interaction), `lru-disk-cache` (EUDR validator tiles)
- **Encryption** : `aes-gcm`, `sha2`, `vaultrs` (Vault client)
- **CRDT** : `automerge` (parcelles sync), plus wrapper layer (Rust FFI JS lib)
- **Testing** : `proptest` (property-based), `insta` (snapshot tests)

### 6.2 Dépendances Java à ajouter

**terroir-payment (Spring Boot)** :
- **Spring Cloud** : `spring-cloud-sleuth` (distributed tracing), `spring-cloud-stream` (Kafka consumer)
- **Mobile money** : **Question P0 : existe-t-il SDK FASO ou from-scratch** ? Hypothèse `kong/gateway-plugins` pour orchestration fallback provider
- **Cache** : `spring-data-redis` (KAYA)
- **Audit** : `shared/audit-lib` déjà intégré

### 6.3 Extensions PostgreSQL requises

- **PostGIS 3.4+** : Parcelles géospatiales (polygones, indexation R-tree), `ST_HausdorffDistance`, opérations géométriques
- **pgcrypto** : Chiffrement column `pgp_sym_encrypt` (backup plan si Vault Transit indisponible)
- **pg_partman** : Partitioning `terroir_conflict_log`, audit-log par date (rétention 5 ans)
- **uuid-ossp** : UUID generation (au cas où Postgres < 13 sans `gen_random_uuid()` native)

### 6.4 Services externes / SDKs

**USSD Providers** :
- `hub2-sdk-rust` (Crate custom ou HTTP API wrapper, afficher pricing)
- `africa-talking-sdk-rust` (idem)
- `twilio-sdk` (Crate `twilio-async`)

**Geospatial datasets** :
- Hansen GFC mirror : S3 public bucket ou FASO self-hosted MinIO
- JRC TMF mirror : idem

**Mobile money** :
- Orange Money BF API SDK (à valider existence + licence)
- Wave.com API
- Moov API
- MTN mPesa API

### 6.5 TypeScript / RN dépendances

**terroir-mobile-bff (Rust), terroir-web-admin (React), terroir-buyer-portal (Next.js)** :
- **RN + Expo** : `expo@52`, `react-native@0.75`
- **CRDT frontend sync** : `automerge` (JS lib), `yjs` (alternative)
- **Maps** : `react-native-maps` ou `mapbox-gl-native` (Android Go benchmark requis)
- **Forms** : `react-hook-form`, `zod` (validation schéma partagé backend)
- **HTTP** : `axios` + custom retries + offline queue (`redux-persist` + local DB)

### 6.6 Infrastructure services à configurer

| Composant | Statut existant | Action TERROIR |
|-----------|---|---|
| PostgreSQL 17 | Running | Ajouter PostGIS 3.4, pg_partman, audit config |
| Vault | Running | Configurer Transit engine `terroir-pii-master` + policies |
| Redpanda | Running | Topics `terroir.*` (member, harvest, payment) + CDC |
| KAYA | Running | Namespacing `terroir:*` (sessions, idempotency, locks) |
| ARMAGEDDON | Running | Routes `/api/terroir/*` + ext_authz Keto |
| MinIO | (implicite) | Buckets `terroir-t-<slug>` + object-lock 5 ans |
| ClickHouse | (implicite) | Créer sink tables agrégats M&E depuis Redpanda CDC |

---

## SECTION 7 — RECOMMANDATIONS DE PHASAGE

### 7.1 Rationale séquencement

**Plutôt que** attaquer 12 modules en parallèle (intenable 6 ETP), **proposé** : slices verticales chaque phase qui :
1. Maximisent early wins (traction avec clients pilote)
2. Valident choix architecturaux (CRDT, multi-tenancy, EUDR validator) sur data réelle
3. Réutilisent au max infra FASO avant inventer du nouveau

### 7.2 Séquencement de phase détaillé

#### Phase P0 (6 semaines) — Discovery & validation architecturale

**Livrables** : (PLAN-TERROIR.md §8 P0)
- [ ] 10 interviews clients (agent terrain, gestionnaire union, exportateur) → user stories validées
- [ ] Design system (Figma) : écrans agent offline + USSD flows
- [ ] 6 ADRs finalisés, feedback intégré (1 par semaine, revues itératives)
- [ ] Port-policy `INFRA/port-policy.yaml` **déjà fait** (terroir-core:8830-8835, terroir-admin:9904)
- [ ] 3 LOI lettres d'intention : 1 exportateur + 1 union + 1 bailleur
- [ ] Containerfile + entry `INFRA/docker/compose/podman-compose.terroir.yml` (bootstrap skeleton)
- [ ] Spec Playwright E2E squelette `tests-e2e/19-terroir/` (happy path agent + USSD)

**Qui** : Tech lead (ADRs, architecture) + BizDev (LOI) + UX designer low-literacy (flows)

**Questions à clarifier P0** : §5 (zones grises) — particulièrement §5.1 (tables exhaustives), §5.2 (offline auth), §5.3 (USSD OTP)

**Go/No-Go** : 3 LOI signées → autorisation Go P1

#### Phase P1 (12 semaines) — MVP core (modules 1, 2, 3)

**Scope** : Registre membres + Cartographie parcelles + Conformité EUDR

**Modules** :
1. **Registre membres** (terroir-core Rust port 8830) : CRUD producteur (nom, CNIB, photos, GPS domicile), KYC validation, parts sociales
2. **Cartographie parcelles** (terroir-core) : Polygones GPS (PostGIS + CRDT Automerge), surface calcul, cultures, historique, notes agronome
3. **Conformité EUDR** (terroir-eudr Rust port 8831 + validateur geospatial) : Ingest parcelles, validation Hansen GFC + JRC TMF cut-off 2020, DDS generation (schéma UE v1.4), signature + submit TRACES NT, evidence archive MinIO

**Frontend** :
- terroir-mobile-bff (BFF RN, port 8833) : App agent terrain offline-first RN+Expo ; formulaires CNIB capture, GPS avec fallback liste, photos compression local
- terroir-web-admin React : Back-office union ; dashboard KYC validation, parcelles carte interactive, export DDS preview

**Infrastructure** :
- [ ] PostgreSQL schema `terroir_shared` + `terroir_t_pilot` (1 coopérative pilote)
- [ ] PostGIS configuré, indices R-tree, test queries spatiales
- [ ] Vault Transit `terroir-pii-master` configuré, DEK envelope tested
- [ ] KAYA namespacing `terroir:session:*`, `terroir:idempotent:*` ready
- [ ] Redpanda topics `terroir.member.*`, `terroir.harvest.*` + CDC schema
- [ ] Flyway migrations `V200__create_terroir_schema.sql` (schema partagé)
- [ ] audit-lib integration (membres + parcelles audit)

**Acceptance** : (PLAN-TERROIR.md §8)
- [ ] 500 producteurs enregistrés (200 en coop pilote, 300 "test"), validité >= 80%
- [ ] 100 parcelles validées >= 95% accuracy geospatial validator (vs. agronome ground truth)
- [ ] 100 DDS générées + submittes TRACES NT, >= 99% accepted (< 1% reject)
- [ ] App RN apk ≤ 25 MB, cold start ≤ 3s Tecno Spark Go, sync 50 livraisons ≤ 2 min EDGE
- [ ] Pen test RSSI (cross-tenant isolation, PII encryption, Vault secret rotation)

**Go/No-Go** : Exportateur pilote signe contrat SaaS production → Go P2

**Effort** : Tech lead (Rust core 1 ETP) + 1 Backend Rust (EUDR validator) + 1 Frontend RN+React (2 ETP) + Agronome/SME EUDR (0.5 ETP) + support BizDev (0.5 ETP) = **5 ETP × 12 sem**

---

#### Phase P2 (10 semaines) — Récolte + intrants + paiement (modules 4, 5, 6)

**Scope** : Traçabilité récolte complète, distribution intrants crédit, paiements mobile money

**Modules** :
4. **Traçabilité récolte** (terroir-core) : Lots/sous-lots, livreurs BLE balance, QR codes, photos, workflow approbation
5. **Gestion intrants** (terroir-core) : Catalogue semences/engrais, distribution crédit agriculteur, remboursement tracking
6. **Paiements mobile money** (terroir-payment Java Spring port 8832 **nouveau service**) : Orange Money BF, Wave, Moov, MTN ; idempotency cache KAYA, réconciliation CDC, notification SMS/USSD (notifier-ms)

**Frontend** :
- terroir-mobile-bff : Flows saisie livraison (poids, photo, GPS), scanner balance BLE, QR codes, offline queue
- terroir-web-admin : Payment dashboard, réconciliation interface, intrants inventory

**Infrastructure** :
- [ ] terroir-payment Java service bootstrap + Kafka producer (paiements → topic `terroir.payment.*`)
- [ ] Mobile money provider SDK integration (Orange/Wave/Moov) — **Question P0** sur réutilisation poulets-platform
- [ ] KAYA idempotency key cache TTL 24h, tested double-payment rejection
- [ ] Redpanda CDC : terroir-core tables → topic `terroir.harvest.*`, `terroir.payment.*`
- [ ] notifier-ms channel extenstion (SMS/USSD producteur payment confirmations)
- [ ] ClickHouse materialized view (early analytics : paiements par région, moyennes)

**Acceptance** :
- [ ] 1 campagne agricole complète (saisie → récolte → paiement en sequence), sans incident grave
- [ ] 50M CFA de paiements traités (cumul 500 producteurs × 100k CFA moyenne), < 0.1% loss
- [ ] Mobile money provider latency p95 ≤ 8 sec (SLO PLAN §7)
- [ ] 0 double-payment detected (idempotency tested 1000 replays)
- [ ] SMS delivery >= 98% (SNR rapport notifier-ms)

**Go/No-Go** : ARR 50 k€ (clients coops payants + donors) → Go P3

**Effort** : Backend Java (paiement + intégrations 1 ETP) + Backend Rust (harvest + intrants 1 ETP) + Frontend (RN forms 1 ETP) + DevOps (Kafka CDC setup 0.5 ETP) + support (0.5 ETP) = **4 ETP × 10 sem**

---

#### Phase P3 (8 semaines) — Marketplace + reporting (modules 7, 8, 9)

**Scope** : Comptabilité SYSCOHADA, marketplace acheteurs (contrats, signature électronique), reporting bailleurs M&E

**Modules** :
7. **Comptabilité OHADA** (terroir-core) : Journaux SYSCOHADA, comptes d'exploitation, exports Sage/Tompro
8. **Marketplace acheteurs** (terroir-buyer Rust port 8835 **nouveau service**) : Annonces lots, contrats, escrow, signature électronique PKI
9. **Reporting bailleurs** (ClickHouse views + terroir-web-bailleur React) : LogFrame indicateurs M&E, DHIS2 export, dashboards BM/AFD/USAID, agrégats anonymisés

**Frontend** :
- terroir-buyer-portal (Next.js) : Public web portal acheteurs, search lots, download DDS, contract signature
- terroir-web-bailleur (React) : Donor dashboards, indicateurs temps réel, export rapports

**Infrastructure** :
- [ ] Multi-tenancy mature : schema-per-tenant (≥5 coops) validation, migration orchestration, backup per-tenant tested
- [ ] Vault PKI (signature électronique contrats), X.509 certs avec HSM option
- [ ] ClickHouse ingest full depuis Redpanda (daily batch + real-time), views `terroir_donor_*`
- [ ] Reporting API REST (terroir-admin) pour dashboards SaaS

**Acceptance** :
- [ ] 1 contrat acheteur signé (BM-financed coop vend à exporter) ; dupliquable
- [ ] 50 coops non-pilote can be deployed via terroiroperating sans incident (scaling)
- [ ] Break-even operationnel 1 coop (ARPU > COGS)
- [ ] Bailleurs rapports generated daily autonomously (zero manual Excel)

**Go/No-Go** : Unit economics align (pricing/cost ratio) → Go P4

**Effort** : Backend Rust (buyer marketplace 0.5 ETP) + Frontend web (Next.js + dashboards 1.5 ETP) + DevOps (ClickHouse, multi-tenant ops 1 ETP) + finance/compliance (SYSCOHADA 0.5 ETP) = **3.5 ETP × 8 sem**

---

#### Phase P4 (12 semaines) — Scale 5 coops + SRE (multi-tenant runbooks)

**Scope** : Consolider multi-tenancy, support tier-1, SLO validation

**Livrables** :
- [ ] 5-10 coops clients operational (paying), 10k producteurs
- [ ] SLO achieve 99.5% DDS submission success, 99.9% core API availability
- [ ] Runbooks (incident response, deployment, rollback, disaster recovery)
- [ ] Support tier-1 training (coops staff)
- [ ] Pen test external (vulnerabilité assessment)

**Go/No-Go** : 10k producteurs live + 99.5% SLO → Go P5

#### Phase P5 (12 semaines) — Formation + crédit + assurance (modules 10, 11)

**Scope** : MOOC, crédit scoring, assurance climatique

Modules 10-11 : partenariat SFD (Small Finance Deed), ARC (African Risk Capacity) ; KYC scoring ; assurance index paramétrique

#### Phase P6 (16 semaines) — Marché du carbone + multi-pays (module 12 + expansion)

**Scope** : MRV (Monitoring Reporting Verification), Verra/Gold Standard credit generation, tokenisation Blockchain (optional, P6+ stretch)

Déploiement replique : Côte d'Ivoire (CI) + Sénégal (SN) + Mali (ML) + Bénin (BJ)

### 7.3 Dépendances inter-phases

```
P0 ────────────────────────────────── (3 LOI, business validation)
  ↓
P1 ─────────── (modules 1,2,3: core + EUDR) ──── (500 producteurs + 100 DDS, exporter LOI)
  ↓
P2 ─────────── (modules 4,5,6: harvest+payment) ──── (50M CFA paid, ARR 50k€)
  ↓
P3 ─────────── (modules 7,8,9: marketplace+reporting) ──── (5 coops, unit economics)
  ↓
P4 ─────────── (scale + SRE) ──── (10k producteurs, 99.5% SLO)
  ↓
P5 ─────────── (formation + crédit)
  ↓
P6 ─────────── (carbone + multi-pays)
```

### 7.4 Justification séquencement

**Pourquoi P0 d'abord** : Business validation + ADRs finalisés réduit risque architectural pivots tard (coûteux en P1+)

**Pourquoi P1 = modules 1,2,3 core** : Modules membres + parcelles = données fondations tout le reste. EUDR validator P1 valide geospatial strategy (Hansen, CRDT) sur data réelle (500 producteurs).

**Pourquoi P2 = harvest+payment** : Récolte = noyau value proposition (traçabilité) ; paiement = monetization (producteur satisfaction). Une campagne complète = proof-of-concept opérationnel.

**Pourquoi P3 = marketplace** : Contractualisation exportateur = garantit SLA (sinon, clients internes uniquement). Multi-tenancy mature au P3 (5+ coops tested) → risque déploiement P4 minimal.

**Pourquoi P4 = scale + SRE** : 10k producteurs + 99.5% SLO = premier mark de vraie opération 24/7. Runbooks + support tier-1 → réplicable pour P5+

**Pourquoi formation/crédit/carbone après** : Modules 10-12 = revenu supplémentaire post-product-market-fit (PMF P1-P3), plutôt qu'avant.

---

## CONCLUSION & PROCHAINES ÉTAPES

### Points clés du rapport

1. **TERROIR architecture** solide, respecte principes FASO souveraineté, réutilise Kratos/Keto/KAYA/Vault/Redpanda existant
2. **Zones grises critiques** (§5) — environ 16 questions précises doivent être clarifiées P0 avant inception P1
3. **Cross-références positives** : Kratos auth, Keto ABAC-capable (extension minor), KAYA sessions, audit-lib, Vault Transit
4. **Nouveautés architecturales** : Schema-per-tenant PG (complexité +), CRDT mobile (learning curve +), envelope encryption (latency +), multi-provider USSD (vendor mgmt +)
5. **Phasage proposé** : P0 discovery → P1 MVP core (500 producteurs, 100 DDS) → P2 récolte+paiement (50M CFA) → P3 marketplace → P4 scale (10k prod, SLO 99.5%) → P5-P6 extensions
6. **Équipe minimale** : 6 ETP an 1 (Tech lead Rust, Backend Java, 2 Frontend, Agronome SME, BizDev), coût ~250k€/an

### Prochaines étapes immédiatement post-rapport

- [ ] **Relecture + feedback users** : Valider rapport avec agent terrain pilote, gestionnaire union, exportateur (→ +1-2 sem)
- [ ] **Workshop zones grises P0** : Réunion 2j avec Tech lead + Agronome + juriste EUDR → clarifier §5 questions exactes (→ +2-3 sem)
- [ ] **LOI signature** : 3 lettres d'intention commerciales (exportateur, union, bailleur) (→ +4-6 sem)
- [ ] **Bootstrap infrastructure P0** : Containerfile, port allocation, Vault Transit setup, Redpanda topics (→ +2 sem en parallèle BizDev)
- [ ] **Démarrage P1** : Une fois 3 LOI signées + tech setup complet

### Coûts & timeline

| Phase | Durée | Team | Budget estimation | Coût cumulé |
|-------|-------|------|---|---|
| P0 | 6 sem | 2 ETP | 50 k€ | 50 k€ |
| P1 | 12 sem | 5 ETP | 125 k€ | 175 k€ |
| P2 | 10 sem | 4 ETP | 100 k€ | 275 k€ |
| P3 | 8 sem | 3.5 ETP | 87 k€ | 362 k€ |
| P4 | 12 sem | 3 ETP (+SRE) | 100 k€ | 462 k€ |

*Estimation hypothétique; à affiner après workshops P0*

---

**Fin du rapport.**
