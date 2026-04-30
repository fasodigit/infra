<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# ADR-004 — Schéma EUDR Due Diligence Statement (DDS) et versioning

| Champ | Valeur |
|---|---|
| Statut | Proposé |
| Date | 2026-04-30 |
| Décideurs | Tech lead, juriste, agronome SME EUDR |
| Contexte | Module 3 TERROIR — génération et soumission DDS conforme TRACES NT |

## Contexte

Le règlement EUDR (UE) 2023/1115 impose à tout opérateur mettant sur le marché UE des produits couverts (cacao, café, soja, bois, palme, bétail, caoutchouc — extensions probables) de déposer une **Due Diligence Statement (DDS)** dans le système informatique **TRACES NT** de la Commission Européenne.

La DDS contient :
- Identifiant opérateur (EORI)
- Description produit (HS code, quantité, unité)
- Liste géolocalisée des plots de production (point GPS si <4 ha, polygone si ≥4 ha)
- Pays de production
- Période de récolte
- Évaluation du risque + mesures de mitigation
- Référence DDS amont (chaîne de traçabilité)

### Contraintes
- Format : JSON conforme schéma UE versionné
- API TRACES NT : OAuth2 + certificats X.509 mTLS
- Conservation 5 ans (article 12 EUDR)
- Schéma UE évolue (v1.0, v1.2, v1.4 actuelle, v1.5 attendue mi-2026)
- Erreur de soumission = lot bloqué à la douane = perte commerciale

### Risques spécifiques
- Schéma UE peut changer en cours de pilote (rétro-incompatibilité)
- Doc EU TRACES NT incomplète, certains champs ambigus
- Validation côté UE asynchrone (jusqu'à 24h)
- Pas de mode test généralisé hors pré-prod sous tutelle

## Options envisagées

### Option A — Modèle interne unique, mappé tardivement vers DDS UE
**Pour** : flexibilité interne.
**Contre** : risque divergence avec schéma UE non détectée, mapping fragile.

### Option B — Modèle interne = miroir du schéma UE (vendor lock-in)
**Pour** : pas de mapping, validation directe.
**Contre** : verrouille TERROIR sur les choix UE, casse à chaque révision schéma.

### Option C — Modèle interne stable + adaptateurs versionnés vers DDS UE
**Pour** : modèle interne survit aux révisions UE, on encapsule la complexité dans un mapper versionné.
**Contre** : un peu plus de code (gérable).

### Option D — Service externe de soumission (BlockMark, Sourcemap, etc.)
**Pour** : aucun maintien schéma.
**Contre** : dépendance forte, coût $$, perte de souveraineté, pas un moat.

## Décision

**Option C — Modèle interne stable + adaptateurs versionnés vers DDS UE.**

### Modèle interne (extrait)
```
TerroirDdsContext {
  operator: { eori, name, country, contact }
  product: { hs_code, description, quantity, unit }
  plots: [
    { id, type: "point" | "polygon", geometry, area_ha,
      country_iso2, harvest_period, deforestation_check }
  ]
  upstream_dds_refs: [String]  // chaîne en amont
  risk_assessment: { level, mitigations: [String] }
  collected_at: timestamp
}
```

### Mapper versionné
- Crate Rust dédiée : `terroir-eudr-dds-mapping`
- Fichiers : `mapping/v1_4.rs`, `mapping/v1_5.rs` (à venir), `mapping/test_corpus/`
- Trait : `DdsMapper` avec `fn render(ctx: &TerroirDdsContext) -> serde_json::Value`
- Tests : snapshots + corpus de DDS de référence (validés par juriste EUDR)
- Sélection runtime : env var `TERROIR_DDS_SCHEMA_VERSION` (défaut latest stable)

### Stockage
- DDS générée stockée en :
  1. PostgreSQL (table `eudr_dds`) : metadata, statut, ref TRACES NT, version schéma
  2. MinIO (objet immuable, S3 object-lock 5 ans) : payload JSON intégral signé Ed25519
- Hash SHA-256 du payload stocké en PG → audit non-tampering

### Soumission TRACES NT
- Service `terroir-eudr` : worker async, retry exponentiel 5/15/60/240 min
- Idempotency key = UUID v7 généré localement, soumis comme `client_reference`
- Statut suivi : `draft` → `validating` → `submitted` → `accepted` / `rejected`
- Webhooks TRACES NT (si dispo) ou polling /status
- Notifications (email + USSD si rejet)

### Validation locale avant envoi
- JSON Schema embedded (copié de UE), validation via `jsonschema` crate
- Validation géom : surface plot ≥ seuil (selon culture), polygone fermé, pas d'auto-intersection
- Validation cut-off déforestation (cf `ADR-eudr-validator-spike` design)
- Validation HS code (contre liste blanche EUDR)

### Compatibilité ascendante / descendante
- Pas de DB schema lock-in : modèle interne reste stable
- Si UE casse v1.4 → v1.5, on ajoute le nouveau mapper, on migre via backfill batch (à chaud, lots non encore soumis)
- DDS soumises restent dans leur version d'origine (immutable) — cf object-lock S3

## Conséquences

### Positives
- Indépendance du schéma UE (encapsulation)
- Audit & rejouabilité (tous les payloads conservés)
- Mapper versionné = unité de migration simple
- Conformité 5 ans assurée (object-lock)

### Négatives
- Code mapper à maintenir à chaque révision UE (~1-2 sem/release)
- Snapshot tests à mettre à jour si UE évolue

### Mitigations
- Veille proactive (newsletter EU EUDR, ESCO observatoire)
- Corpus de tests partagé avec partenaires (autres opérateurs UEMOA pour mutualiser)
- CI dédié au mapper (cargo test --features eudr-validate-corpus)

## Sécurité

- Certificats X.509 client TRACES NT en Vault PKI
- Rotation 1 an (cycle UE)
- mTLS obligatoire toutes connexions TRACES NT
- Audit log immuable (Loki + MinIO object-lock 5 ans) toutes soumissions

## Métriques de succès

- DDS submission success rate ≥ 99.5% / 30j
- 0 lot bloqué douane pour cause DDS rejetée
- Latence end-to-end (DDS générée → accepted) p95 ≤ 24h
- Migration v1.4 → v1.5 < 1 sprint sans interruption service

## Révision

À reconfirmer après publication schéma UE v1.5 (attendu mi-2026). Validation par juriste EUDR externe avant production.
