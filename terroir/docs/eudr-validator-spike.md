<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# EUDR Validator — Spike de design (sans code)

| Champ | Valeur |
|---|---|
| Statut | Design (no code yet) |
| Date | 2026-04-30 |
| Auteur | LIONEL TRAORE |
| Cible | Crate Rust `terroir-eudr-validator` (workspace `INFRA/terroir/`) |
| Lié à | ADR-004 (DDS schema), Module 3 du PLAN-TERROIR |

## 1. Objectif

Concevoir (sans coder) le module qui prend en entrée :
- Un **polygone GPS** (ou point si <4 ha) de parcelle
- Une **date de récolte**

… et qui retourne :
- `deforestation_post_2020: bool` — la parcelle a-t-elle été défrichée après le 31/12/2020 ?
- `score: f64` — proportion défrichée (0.0 à 1.0)
- `evidence: { tile_url, dataset_version, computed_at }` — preuves vérifiables
- `risk_level: Low | Medium | High` — synthèse heuristique pour DDS

C'est le composant le plus risqué du MVP : si la validation est fausse, tout l'édifice EUDR s'écroule. D'où l'importance de poser le design avant de coder.

## 2. Sources de données

### 2.1 Hansen Global Forest Change (UMD/GLAD)
- **Producteur** : University of Maryland, Hansen et al.
- **Résolution spatiale** : 30 m (Landsat-derived)
- **Couverture temporelle** : 2000 → 2024 (mises à jour annuelles)
- **Format** : GeoTIFF, tuiles 10°×10°
- **Bande clé** : `lossyear` — année de perte forestière (1-24, 0 = pas de perte)
- **Taille totale** : ~50 GB compressé (monde entier)
- **Licence** : usage non-commercial libre, commercial avec attribution
- **URL** : https://glad.umd.edu/dataset/global-forest-change

### 2.2 JRC TMF (Tropical Moist Forests) — recommandé Commission UE
- **Producteur** : Joint Research Centre (Commission UE)
- **Résolution** : 30 m
- **Spécificité** : tropical moist forests (== zone agricole UEMOA pertinente)
- **Bandes** : `transition` (12 classes), `deforestation_year`
- **Licence** : ouverte (PSI 2019)
- **URL** : https://forobs.jrc.ec.europa.eu/TMF
- **Avantage EUDR** : recommandé par la DG ENV comme dataset autoritaire

### 2.3 Comparatif et stratégie
| Critère | Hansen GFC | JRC TMF |
|---|---|---|
| Reconnaissance UE | Forte | **Officielle** |
| Couverture sahel/savane | Bonne | Limitée (TMF = forêts humides uniquement) |
| Mise à jour | Annuelle | Annuelle |
| Précision sahel/savane | Moyenne | Faible (zones non couvertes) |

**Décision (à reconfirmer en P0)** : utiliser les deux sources avec règle de priorité :
1. Si JRC TMF couvre la zone → autoritaire pour conformité
2. Sinon Hansen GFC → preuve auxiliaire
3. Si désaccord → flag manuel + revue agronome

### 2.4 Sources complémentaires (P2+)
- **Planet Labs** (commercial) : 3 m / journalier, partenariat possible avec ESA/UE pour accès subventionné
- **Sentinel-2** (Copernicus, gratuit) : 10 m / 5 j → vérification ad hoc
- **Imagery NASA SERVIR** (sahel-spécifique) : à explorer

## 3. Architecture du validateur

### 3.1 Composants logiques

```
┌──────────────────────────────────────────────────────┐
│ terroir-eudr-validator (Rust crate)                  │
│                                                      │
│ ┌──────────────────────────────────────────────────┐ │
│ │ public API                                       │ │
│ │  validate(polygon, harvest_date) -> Outcome      │ │
│ └─────────────────┬────────────────────────────────┘ │
│                   ▼                                  │
│ ┌──────────────────────────────────────────────────┐ │
│ │ TileFetcher                                      │ │
│ │  - cache local (LRU disk, 5 GB)                  │ │
│ │  - fallback HTTP (S3 signed URL UMD/JRC mirror)  │ │
│ │  - vérification hash SHA-256 (immuabilité)       │ │
│ └────────────┬───────────────┬─────────────────────┘ │
│              ▼               ▼                       │
│   ┌─────────────────┐ ┌─────────────────┐            │
│   │ HansenAdapter   │ │ JrcTmfAdapter   │            │
│   │ - read lossyear │ │ - read transit  │            │
│   └────────┬────────┘ └────────┬────────┘            │
│            ▼                   ▼                     │
│ ┌──────────────────────────────────────────────────┐ │
│ │ Reasoner                                         │ │
│ │  - cut-off check 2020-12-31                      │ │
│ │  - polygon clip + sample                         │ │
│ │  - majority rule + agreement entre sources       │ │
│ │  - risk_level heuristic                          │ │
│ └─────────────────┬────────────────────────────────┘ │
│                   ▼                                  │
│ ┌──────────────────────────────────────────────────┐ │
│ │ Outcome { deforestation_post_2020, score, ... }  │ │
│ └──────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

### 3.2 Crates Rust pressenties (à valider)
- `gdal-rs` ou `geozero` + `geo` : lecture GeoTIFF, opérations géométriques
- `proj` : reprojections (parcelles WGS84 → tile EPSG)
- `serde` + `serde_json` : DDS payload
- `jsonschema` : validation schema UE
- `reqwest` + `tokio` : fetch tuiles
- `lru-disk-cache` : cache local
- `sha2` : vérification intégrité tuiles

### 3.3 Algorithme (pseudo-code, sans code Rust)
```
fn validate(polygon, harvest_date):
    cut_off = 2020-12-31
    
    # 1. Bounding box
    bbox = polygon.bounds()
    
    # 2. Tuiles à charger
    tiles_hansen = TileFetcher.tiles_for_bbox(bbox, dataset=HANSEN)
    tiles_jrc    = TileFetcher.tiles_for_bbox(bbox, dataset=JRC_TMF)
    
    # 3. Pour chaque pixel intersectant le polygone :
    for pixel in polygon.intersect(tiles_hansen):
        if pixel.lossyear > 20:  # i.e. perte après 2020
            mark_deforested(pixel, source=HANSEN)
    
    for pixel in polygon.intersect(tiles_jrc):
        if pixel.deforestation_year > 2020:
            mark_deforested(pixel, source=JRC_TMF)
    
    # 4. Score = surface défrichée / surface totale parcelle
    score = sum(pixel.area for pixel in deforested) / polygon.area
    
    # 5. Règle décision
    if jrc_tmf_covers(polygon):
        deforestation_post_2020 = (jrc_score > 0.0)
    else:
        deforestation_post_2020 = (hansen_score > 0.05)  # tolérance 5% bruit
    
    # 6. Risk level (heuristique pour DDS)
    risk_level = match score:
        0.0..=0.01  -> Low
        0.01..=0.10 -> Medium
        _           -> High
    
    return Outcome {
        deforestation_post_2020,
        score,
        sources: [HANSEN, JRC_TMF if applicable],
        evidence: { tile_urls, dataset_versions, computed_at },
        risk_level,
    }
```

### 3.4 Cas limites à traiter explicitement
| Cas | Stratégie |
|---|---|
| Polygone à cheval sur 2 tuiles | Fusion seamless via `geo::ops::union` |
| Polygone très petit (< 1 px) | Erreur `PolygonTooSmall`, exiger surface ≥ 0.0009 km² (1 px Hansen) |
| Hansen et JRC en désaccord | `Disagreement` flag, revue manuelle, conservé en evidence |
| Datasets mis à jour pendant batch | Hash SHA-256 figé en début de campagne, alerte si rotation |
| Polygone hors zone tropical (sahel sec) | JRC TMF retourne « no data » → fallback Hansen, flag `coverage: hansen-only` |
| Polygone auto-intersection ou trou | `InvalidGeometry` rejeté upstream avec message clair |
| Date de récolte incohérente (futur) | `InvalidHarvestDate`, refus immédiat |

## 4. Plan de validation (acceptance)

### 4.1 Corpus de test (à constituer P0)
- 100 parcelles BF coton (validées terrain par agronome)
- 50 parcelles CI cajou (avec déforestation post-2020 connue)
- 50 parcelles SN sésame
- 20 cas adversariaux (polygones < 1 ha, à cheval frontière, multi-polygones)
- Snapshots de DDS attendus pour chaque cas → tests `insta` + corpus golden

### 4.2 KPI
- Latence p95 ≤ 300 ms par parcelle (cache chaud)
- Latence p95 ≤ 5 s (cache froid, tuile à fetch)
- Précision (agronome ground truth) ≥ 95% sur corpus de référence
- 0 faux négatif sur cas connus de déforestation post-2020

### 4.3 Bench
- 10k parcelles batch → < 10 minutes (avec cache chaud)
- Mémoire ≤ 1 GB sur le worker
- Concurrent safety : 100 validations parallèles, 0 corruption cache

## 5. Stratégie de cache

### 5.1 Niveaux
1. **Mémoire L1** : LRU 100 tuiles décodées (mmap + interior mut) → instant
2. **Disque L2** : tuiles GeoTIFF brutes, 5 GB max, eviction LRU
3. **Réseau L3** : miroir S3 souverain (MinIO) + fallback UMD/JRC originaux

### 5.2 Pré-chargement
- Job nightly : pré-charge tuiles couvrant les zones où on a des coopératives actives
- Statistiques cache hit ratio dans Prometheus (`terroir_eudr_cache_hits_total`)

### 5.3 Invalidation
- Datasets mis à jour annuellement (Hansen v1.x, JRC TMF v1.x)
- Stratégie : version explicite dans le path (`hansen/v1.11/lossyear/N00E000.tif`)
- Job de rotation : nouvelle version → re-validation différentielle des parcelles → alerte si re-classification

## 6. Sécurité & confiance

- **Intégrité** : hash SHA-256 vérifié de chaque tuile (catalogue signé Ed25519 publié par UMD/JRC ou nous-mêmes)
- **Audit** : chaque validation produit un `Outcome` avec evidence URLs **immuables** (S3 object-lock 5 ans)
- **Reproductibilité** : `Outcome.dataset_version` permet de re-jouer une décision passée → robustesse litige
- **Pas de secrets en clair** : token MinIO en Vault `faso/terroir/eudr/minio-key`
- **Logs** : aucun PII, aucune coordonnée GPS précise au-delà du strictement nécessaire

## 7. Évolutions prévues (post-MVP)

| Version | Apport |
|---|---|
| v0.1 | Hansen + JRC TMF, validation parcelle-par-parcelle |
| v0.2 | Batch optimisé, cache pré-chauffé |
| v0.3 | Sentinel-2 ad hoc pour vérifications litige |
| v0.4 | ML model pour classification automatique cultures (coton vs jachère) |
| v1.0 | Partenariat Planet Labs pour 3m / journalier |

## 8. Risques

| Risque | P | I | Mitigation |
|---|---|---|---|
| Hansen ou JRC mis à jour avec rétro-changement | M | H | Conservation versions, rejouabilité, alarme delta > 5% |
| GeoTIFF corruption (réseau / disque) | L | H | SHA-256 obligatoire avant lecture |
| Bug OGC dans `geo` Rust crate | L | H | Property-based tests proptest, corpus golden |
| Performance batch insuffisante | M | M | Profiling P1, parallélisation tokio rayon |
| Désaccord Hansen vs JRC fréquent | M | M | Workflow revue agronome, KPI désaccord ≤ 5% |
| Coût stockage si large couverture | L | L | Cache LRU + delete after 90j si tenant inactif |

## 9. Hors scope (non couvert ici)

- Soumission TRACES NT (cf ADR-004 + module distinct `terroir-eudr-submitter`)
- Génération du payload DDS JSON (mapper séparé)
- UI de revue agronome (cf terroir-web-admin)
- Notifications utilisateur (terroir-notifier)

## 10. Prochaines étapes (après validation business case)

1. POC Rust 1 semaine : fetch 1 tuile Hansen + 1 tuile JRC, intersect 1 polygone test connu, vérifier output
2. Constitution du corpus de référence (10 parcelles ground truth) avec agronome SME
3. Décision finale `gdal-rs` vs `geozero` (perf + ergonomie)
4. Mise en place miroir S3 MinIO des tuiles (license-compliant)
5. Bench mémoire / latence sur 1k parcelles
6. Crate publiée en `INFRA/terroir/crates/eudr-validator/`
7. Intégration dans `terroir-eudr` service (port 8831)

---

**Note** : ce document est purement de design. Aucun code Rust n'a encore été écrit conformément à la consigne « pas de codes à implémenter actuellement ». La crate sera scaffoldée en début de P1 après validation des LOI commerciales et discovery (P0).
