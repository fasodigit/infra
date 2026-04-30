<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# TERROIR — Plan stratégique de digitalisation des coopératives agricoles

| Champ | Valeur |
|---|---|
| Statut | Draft v0.1 |
| Auteur | LIONEL TRAORE (FASO DIGITALISATION) |
| Dernière révision | 2026-04-30 |
| Périmètre | Burkina Faso → Afrique de l'Ouest (8 pays UEMOA + Ghana) |
| Filières cibles | Coton, sésame, karité, anacarde (extensions futures : cacao, café, hévéa) |
| Conformité primaire | EUDR (Règlement UE 2023/1115) |

---

## 1. Vision produit

> **Une plateforme souveraine, rurale-first et low-bandwidth qui permet aux coopératives agricoles ouest-africaines de tracer leurs récoltes, payer leurs producteurs et démontrer leur conformité EUDR aux exportateurs UE — pour 5 à 10× moins cher que les solutions internationales.**

### Pourquoi maintenant
- **EUDR** : entrée en vigueur 30/12/2025 (grandes entreprises) et 30/06/2026 (PME). Tout opérateur qui met sur le marché UE coton/cajou/sésame/karité doit prouver l'absence de déforestation post-2020 + géolocalisation.
- **Marché financé** : Banque mondiale (programme STEP), AFD (ARAA, AVENIR), USAID (Trade Hub, Feed the Future), GIZ (ProAgri) ont des fonds dédiés à la traçabilité et au M&E numérique.
- **Stack souveraine FASO mature** : KAYA, ARMAGEDDON, auth-ms, Vault, Redpanda, ORY, observabilité — TERROIR hérite de l'infra et ne la reconstruit pas.

### Différenciation vs concurrents
| Concurrent | Faiblesse exploitable |
|---|---|
| Geotraceability (UK) | Pricing $$$, pas d'hébergement Afrique de l'Ouest, pas de mobile money |
| Farmforce (Syngenta) | Verrouillé sur l'écosystème Syngenta, pas EUDR-natif |
| Sourcemap (US) | UI complexe, pas adapté agent-terrain low-literacy |
| Meridia (NL) | Cher, project-based pas SaaS, pas de USSD |
| Cahier papier | Toujours dominant, double saisie, perte fiches |

**Moat TERROIR** = souveraineté + prix + 7 langues nationales + USSD producteur + mobile money natif + intégration directe stack FASO.

---

## 2. Personas & jobs-to-be-done

### 2.1 Agent collecteur (terrain)
- Ramasse récolte + intrants en brousse, parfois sans réseau pendant 7-14 jours
- Doit identifier le producteur (CNIB ou QR), peser, marquer le lot, GPS-localiser le plot
- **JTBD** : « Quand je suis dans un village sans réseau, je veux pouvoir saisir 50 livraisons rapidement et que tout remonte automatiquement quand je rentre. »

### 2.2 Gestionnaire union/faîtière
- Pilote 5 à 50 coopératives membres, 5k à 50k producteurs
- Suit les engagements intrants, les paiements, les volumes
- **JTBD** : « Je dois préparer le rapport mensuel pour mon bailleur en 1 jour, pas en 1 semaine. »

### 2.3 Acheteur / exportateur
- Achète des lots conditionnés, charge en conteneurs, exporte vers UE
- Doit produire la DDS (Due Diligence Statement) EUDR pour chaque conteneur
- **JTBD** : « Quand un lot arrive à mon entrepôt, je veux générer la DDS en 1 clic et la soumettre à TRACES NT. »

### 2.4 Bailleur / responsable M&E
- Suit les indicateurs LogFrame de son projet (BM, AFD, USAID)
- **JTBD** : « Je veux des indicateurs M&E temps réel auditable, pas des fichiers Excel envoyés par email. »

---

## 3. Modules fonctionnels (12)

| # | Module | Périmètre | MVP |
|---|---|---|---|
| 1 | Registre membres | KYC, CNIB, parts sociales, ménage, genre, âge | P0 |
| 2 | Cartographie parcelles | Polygones GPS, surface, cultures, rotation | P0 |
| 3 | Conformité EUDR | Validation cut-off, DDS, soumission TRACES NT | P0 |
| 4 | Traçabilité récolte | Lots, sous-lots, balances, QR sacs | P1 |
| 5 | Gestion intrants | Catalogue, distribution crédit, remboursement | P1 |
| 6 | Paiements mobile money | Orange/Moov/Wave/MTN, idempotence, réconciliation | P1 |
| 7 | Comptabilité OHADA | SYSCOHADA, journaux, exports Sage/Tompro | P2 |
| 8 | Marketplace acheteurs | Annonces lots, contrats, escrow, signature | P2 |
| 9 | Reporting bailleurs | LogFrame, DHIS2, exports BM/AFD/USAID | P2 |
| 10 | Formation & vulgarisation | MOOC langues nationales, audio, quiz USSD | P3 |
| 11 | Crédit & assurance | Scoring, assurance index climatique | P3 |
| 12 | Marché du carbone | MRV, Verra/Gold Standard, tokenisation | P4 |

---

## 4. Architecture technique (résumé)

```
┌─────────────────── Edge / mobile / USSD ───────────────────┐
│ App agent terrain (RN+Expo, offline 14j, CRDT)             │
│ Portail acheteur web (Next.js)                             │
│ Back-office union (React)                                  │
│ Producteur USSD (Africa's Talking + Hub2 + Twilio)         │
└──────────────────┬──────────────────────────┬──────────────┘
                   │                          │
                   ▼                          ▼
        ┌──────────────────────┐    ┌──────────────────┐
        │ ARMAGEDDON gateway   │    │ terroir-ussd     │
        │ (mTLS, WAF, rate-lim)│    │ :8834            │
        └──────────┬───────────┘    └────────┬─────────┘
                   │                         │
   ┌───────────────┼─────────────────────────┼──────────────┐
   │ terroir-core  │ terroir-mobile-bff      │              │
   │ :8830 (Rust)  │ :8833 (Rust)            │              │
   ├───────────────┼─────────────────────────┼──────────────┤
   │ terroir-eudr  │ terroir-payment         │ terroir-buyer│
   │ :8831 (Rust)  │ :8832 (Java Spring)     │ :8835        │
   └───────┬───────┴────────────┬────────────┴──────┬───────┘
           │                    │                   │
           ▼                    ▼                   ▼
     ┌─────────┐         ┌──────────┐        ┌──────────┐
     │PostGIS  │         │KAYA cache│        │MinIO/S3  │
     │+ FK PG  │         │idempo+sess│       │photos    │
     └────┬────┘         └─────┬────┘        └────┬─────┘
          │                    │                  │
          └────────────────────┼──────────────────┘
                               ▼
                       Redpanda (CDC + outbox)
                               │
                               ▼
                       DuckDB / ClickHouse-souverain
                       (analytics + reporting bailleurs)
```

### Composants
- **terroir-core** (Rust, Axum) — registre membres + parcelles
- **terroir-eudr** (Rust) — validation géospatiale, DDS, TRACES NT
- **terroir-payment** (Java Spring) — orchestration mobile money
- **terroir-mobile-bff** (Rust) — BFF agent terrain
- **terroir-ussd** (Rust) — gateway USSD/SMS
- **terroir-buyer** (Rust) — portail acheteurs

### Données
- PostgreSQL 16 + PostGIS 3.4 (géométries, R-tree, FK transactions)
- KAYA (cache, sessions, idempotency mobile money)
- Redpanda (topics `member.*`, `harvest.*`, `payment.*`, `eudr.dds.*`)
- MinIO/Garage (photos, scans CNIB, justificatifs DDS, archivage 5 ans)
- ClickHouse souverain (analytics)

### Auth
- ORY Kratos (existant) + Keto ABAC (rôle × coop × région)
- Biométrie optionnelle agent (FaceTec ou Innovatrics-like)
- Hardware key (Yubikey) pour admin union/exportateur

---

## 5. Conformité réglementaire

| Domaine | Référence | Action |
|---|---|---|
| EUDR | Règlement (UE) 2023/1115 | DDS schema v1.4, soumission TRACES NT, conservation 5 ans |
| Données personnelles BF | Loi 010-2004/AN | Déclaration CIL, consentement audio |
| Données personnelles UE | RGPD | DPIA biométrie + géoloc, transferts encadrés |
| Mobile money | BCEAO/UEMOA | Partenariat banque (pas d'agrément EMI au démarrage) |
| AML | CENTIF | Reporting transactions ≥ seuil |
| Comptable | SYSCOHADA révisé 2017 | Plan comptable conforme, exports Sage/Tompro |
| Convention de Malabo | UA | Transfert intra-Afrique sans restriction |

---

## 6. Sécurité (résumé)

- mTLS service-to-service (SPIRE, déjà en place)
- Vault KV : `faso/terroir/<service>/<usage>`, policies read-only par service
- pgcrypto + Vault Transit pour PII (CNIB, téléphone, photo) — rotation 90j
- Audit log immuable → Loki + MinIO object-lock 5 ans
- Pen-test semestriel + bug bounty privé (HackerOne / YesWeHack)
- Voir `INFRA/SECURITY.md` pour la politique globale

---

## 7. SLO & observabilité

| SLO | Cible | Fenêtre |
|---|---|---|
| DDS submission success rate | ≥ 99.5% | 30 j |
| Mobile money payment latency p95 | ≤ 8 s | 30 j |
| Sync mobile-agent disponibilité (réseau 2G/3G) | ≥ 99% | 7 j |
| terroir-core API availability | ≥ 99.9% | 30 j |
| EUDR validator latency p95 (1 polygone) | ≤ 300 ms | 7 j |

Métriques Prometheus, traces OTel → Tempo, logs → Loki — infra déjà en place. Dashboards Grafana par coopérative + un global.

---

## 8. Roadmap

| Phase | Durée | Livrables clés | Acceptance |
|---|---|---|---|
| **P0 Discovery** | 6 sem | 10 interviews, design system, ADRs, port-policy | 3 LOI signées |
| **P1 MVP membre+plot+EUDR** | 12 sem | terroir-core, app agent, validateur EUDR | 500 producteurs, 100 DDS |
| **P2 Récolte+intrants+paiement** | 10 sem | Modules 4-5-6, Orange Money + Wave | 50M CFA payés, 1 campagne complète |
| **P3 Marketplace+reporting** | 8 sem | Modules 7-8-9, signature électronique | 1 contrat acheteur signé |
| **P4 Scale 5 coops** | 12 sem | Multi-tenant, runbooks, support tier-1 | 10k producteurs, 99.5% SLO |
| **P5 Formation+crédit+assurance** | 12 sem | Modules 10-11, partenariats SFD + ARC | 2 produits financiers actifs |
| **P6 Carbone+multi-pays** | 16 sem | Module 12, déploiement CI/SN/ML/BJ | 1er crédit carbone |

Jalons go/no-go :
- Fin P1 : pilote validé par exportateur signant un contrat SaaS
- Fin P2 : ARR 50 k€
- Fin P4 : break-even opérationnel d'une coopérative cliente

---

## 9. Modèle économique

| Stream | Pricing | Volume cible an 3 |
|---|---|---|
| SaaS exportateur | 2-5 €/producteur actif/an | 200k → 600 k€ |
| DDS soumission | 0.20 €/DDS | 500k → 100 k€ |
| Implémentation | 15-50 k€ par déploiement | 20 coops → 600 k€ |
| Donor-funded pilots | 100-500 k€ par contrat | 3 → 900 k€ |
| White-label | Royalty 15% | — |

**TAM/SAM/SOM**
- TAM ouest-africain (agri-tech traçabilité) : 200 M€
- SAM (coton + cajou + sésame + karité) : 60 M€
- SOM an 3 : 2.2 M€ ARR

---

## 10. Go-to-market

- **Pilote ancrage** : SOFITEX coton (1.5M producteurs) ou OLAM cajou Côte d'Ivoire — viser 1 LOI dans 90 j
- **Multiplicateurs** : SNV, GIZ, AFC, ICCO (intégrateurs ONG)
- **Donors** : BM STEP, AFD ARAA, USAID Trade Hub, IFC GAFSP
- **Évents** : SARA Abidjan, FIARA Dakar, Africa Agri Forum
- **Pricing transparent + démo 30 min + free trial 60 j**

---

## 11. Risques & mitigations

| Risque | P | I | Mitigation |
|---|---|---|---|
| Adoption agent terrain (illettrisme tech) | H | H | Formation présentielle 3j + IVR + champion local |
| Réseau 2G coupé pendant sync | H | M | Offline 14j, sync delta-encoded |
| Mobile money provider downtime | M | H | Multi-provider failover (Orange + Wave) |
| Erreur géoloc post-2020 | M | H | Re-validation auto trimestrielle + appel humain |
| Concurrent international avec budget 100× | H | M | Souveraineté + prix + langues = moat |
| Litige données producteur | L | H | Consentement audio + DPIA + assurance RC |
| EUDR reporté/annulé | L | H | Module utile pour acheteurs volontaires (Mars, Nestlé, L'Occitane) |
| Dépendance datasets Hansen/JRC | M | M | Cache local + alternative Planet Labs partenariat |

---

## 12. Équipe minimale

| Rôle | ETP | Localisation |
|---|---|---|
| Tech lead Rust/PostGIS | 1 | Ouaga / remote |
| Backend Java/Spring (paiement) | 1 | Ouaga |
| Frontend React + RN/Flutter | 2 | Mix |
| Agronome / SME EUDR | 0.5 | Bobo-Dioulasso |
| Designer UX low-literacy | 0.5 | Dakar |
| BizDev / partenariats | 1 | Abidjan |
| **Total an 1** | **6 ETP** | Budget ~ 250 k€/an |

---

## 13. Décisions architecturales (ADRs)

Voir `INFRA/terroir/docs/adr/`.

- ADR-001 — Mobile framework (RN vs Flutter)
- ADR-002 — Sync & conflict resolution (CRDT vs LWW)
- ADR-003 — USSD provider (multi-provider abstraction)
- ADR-004 — EUDR DDS schema versioning
- ADR-005 — PII encryption (pgcrypto + Vault Transit)
- ADR-006 — Multi-tenancy (schema-per-tenant + Keto + RLS)

---

## 14. Étapes immédiates (sem 1-2)

- [x] Réserver les ports dans `INFRA/port-policy.yaml`
- [x] Rédiger les 6 ADRs structurants
- [x] Spec design du validateur EUDR (sans code)
- [ ] Maquette Figma écrans agent terrain (mode hors-ligne) + parcours USSD
- [ ] Workshop avec 1 coopérative cible → 5 user stories validées
- [ ] LOI commerciale signée (exportateur ou faîtière)
- [ ] Containerfile + entry `INFRA/docker/compose/podman-compose.terroir.yml`
- [ ] Spec Playwright squelette `tests-e2e/19-terroir/`
- [ ] Bootstrap workspace Rust `INFRA/terroir/` (après validation business case)

---

## 15. Références externes

- EUDR — https://eur-lex.europa.eu/eli/reg/2023/1115
- TRACES NT — https://webgate.ec.europa.eu/tracesnt
- Hansen Global Forest Change — https://glad.umd.edu/dataset/global-forest-change
- JRC TMF — https://forobs.jrc.ec.europa.eu/TMF
- SYSCOHADA — http://www.ohada.org
- DDS JSON Schema — Annexe règlement EUDR (à vérifier dernière version au démarrage P0)
