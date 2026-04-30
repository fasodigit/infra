<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# LICENSES-GEO — Attribution datasets géospatiaux mirrorés (TERROIR P1.C)

| Champ | Valeur |
|---|---|
| Statut | Actif |
| Dernière revue | 2026-04-30 |
| Owner | LIONEL TRAORE — TERROIR P1 lead |
| Lié à | `INFRA/scripts/sync-hansen-gfc.sh`, `INFRA/scripts/sync-jrc-tmf.sh`, `INFRA/observability/cronjobs/geo-mirror-sync.yaml` |

## 1. Objet

Ce document recense les licences et obligations d'attribution pour les
datasets géospatiaux externes que FASO DIGITALISATION mirrore dans
MinIO souverain (bucket `geo-mirror`) pour les besoins de validation
EUDR (`terroir-eudr`, P1.B).

**Principe de souveraineté** (CLAUDE.md §3) : ces datasets sont des
*sources de données téléchargées une fois et stockées sur infrastructure
souveraine* — ils **ne sont pas** des services cloud étrangers en
dépendance runtime. Le pipeline de validation EUDR fonctionne offline-first
contre les copies MinIO ; la sync hebdomadaire est best-effort.

## 2. Hansen Global Forest Change (UMD/GLAD)

| Attribut | Valeur |
|---|---|
| Producteur | University of Maryland — GLAD Lab (Hansen et al.) |
| Dataset | Global Forest Change v1.11 (GFC-2024) |
| Résolution | 30 m (Landsat-derived) |
| Couverture | Globale, 2000–2024 |
| Format | GeoTIFF, tuiles 10°×10° |
| Bandes utiles | `lossyear`, `treecover2000`, `datamask` |
| URL source | https://storage.googleapis.com/earthenginepartners-hansen/GFC-2024-v1.11/ |
| Page projet | https://glad.umd.edu/dataset/global-forest-change |

### 2.1 Licence

**Creative Commons Attribution 4.0 International (CC BY 4.0)**
→ https://creativecommons.org/licenses/by/4.0/

Autorise :
- Usage commercial (vente de services SaaS EUDR)
- Reproduction, redistribution, dérivés (mirror MinIO, calculs)
- Modification (clipping, intersect avec polygones)

Obligations :
- **Attribution** : citer Hansen et al. 2013 dans toute évidence DDS
- Indiquer si modifications (clipping/sampling)
- Lien vers la licence

### 2.2 Citation à inclure dans toute évidence DDS

> Hansen, M.C., P.V. Potapov, R. Moore, M. Hancher, S.A. Turubanova,
> A. Tyukavina, D. Thau, S.V. Stehman, S.J. Goetz, T.R. Loveland,
> A. Kommareddy, A. Egorov, L. Chini, C.O. Justice, and J.R.G. Townshend.
> 2013. "High-Resolution Global Maps of 21st-Century Forest Cover Change."
> *Science* 342: 850–853.
> Data available on-line from: <https://glad.umd.edu/dataset/global-forest-change>.

### 2.3 Compatibilité AGPL

CC BY 4.0 est **compatible** avec AGPL-3.0-or-later (OSI/FSF reconnaît la
combinaison ; obligation citation préservée dans le DDS XML/PDF généré
par `terroir-eudr`).

## 3. JRC Tropical Moist Forest (Commission Européenne)

| Attribut | Valeur |
|---|---|
| Producteur | Joint Research Centre (Commission UE), DG ENV |
| Dataset | Tropical Moist Forest v1_2024 |
| Résolution | 30 m |
| Couverture | Zones tropicales humides (équateur ±25°) |
| Format | GeoTIFF, blocs continentaux |
| Bandes utiles | `transition` (12 classes), `deforestation_year` |
| URL source | https://forobs.jrc.ec.europa.eu/TMF/ |
| Endpoint download | https://ies-ows.jrc.ec.europa.eu/iforce/tmf_v1/download |

### 3.1 Licence

**Open Data conformément à la Directive PSI 2019/1024** (équivalente
CC BY 4.0 dans la pratique JRC).
→ https://creativecommons.org/licenses/by/4.0/

Autorise :
- Usage commercial
- Reproduction, redistribution, dérivés
- Mirror MinIO

Obligations :
- **Attribution** au JRC + citation Vancutsem et al. 2021
- Lien vers la source originelle
- Mention « modified » si clipping/sampling

### 3.2 Citation à inclure dans toute évidence DDS

> Vancutsem, C., Achard, F., Pekel, J.-F., Vieilledent, G., Carboni, S.,
> Simonetti, D., Gallego, J., Aragão, L.E.O.C., Nasi, R. 2021.
> "Long-term (1990–2019) monitoring of forest cover changes in the
> humid tropics." *Science Advances* 7, eabe1603.
> DOI: 10.1126/sciadv.abe1603.
> Data: <https://forobs.jrc.ec.europa.eu/TMF>.

### 3.3 Compatibilité AGPL

PSI 2019 / CC BY 4.0 — **compatible** AGPL. Recommandé par DG ENV comme
dataset autoritaire pour la conformité EUDR (Règlement UE 2023/1115).

## 4. Mécanisme d'attribution dans le DDS

`terroir-eudr` doit injecter ces deux blocs d'attribution dans :

1. **Payload DDS XML** (élément `<dataSourceAttribution>`) — soumis à
   TRACES NT.
2. **PDF d'évidence** (footer page de garde) — partagé avec
   l'exportateur et l'autorité BF.
3. **JSON `Outcome.evidence.dataset_versions`** — audit trail interne
   permettant de re-jouer une décision.

Le mapper DDS (P1.B `terroir-eudr-validator`) lit le `MANIFEST.json` du
prefix MinIO mirroré (`hansen-gfc/v1.11/MANIFEST.json` et
`jrc-tmf/v1_2024/MANIFEST.json`) pour récupérer dataset_version,
synced_at et citation. Aucune donnée hardcodée côté code Rust → toute
évolution attribution se propage via re-sync sans rebuild.

## 5. Refresh & versioning

- **Versions figées** : v1.11 Hansen, v1_2024 JRC. Aucune bascule
  automatique vers une version supérieure.
- **Détection drift** : CronJob hebdo (`geo-mirror-sync.yaml`)
  → métrique Prometheus `geo_mirror_version_drift_total{dataset="..."}`
  → alerte SRE si non-zéro.
- **Bascule version** : décision manuelle (LIONEL TRAORE + SME EUDR)
  → bump `HANSEN_VERSION`/`JRC_TMF_VERSION` dans `faso-geo-mirror-config`
  ConfigMap → re-validation différentielle des DDS existants
  (cf. RUNBOOK-GEO-MIRRORS.md §refresh-policy).
- **Rétention immutable** : object-lock S3 5 ans sur `geo-mirror/*` →
  reproductibilité litige (re-jouer une validation passée avec la
  version du dataset utilisée à l'époque).

## 6. Sources additionnelles (non mirrorées en P1)

| Dataset | License | Statut FASO | Phase ciblée |
|---|---|---|---|
| Sentinel-2 (Copernicus) | CC0 | À évaluer P2 | Vérification ad hoc litige |
| Planet Labs | Commercial | Partenariat ESA P4+ | Imagerie 3m journalière |
| NASA SERVIR West Africa | Public domain | À explorer | Sahel-spécifique |

Ces sources seront ajoutées au présent document avec leur licence avant
toute intégration dans `terroir-eudr`.

## 7. Audit & conformité

- Toute fonction de génération DDS dans `terroir-eudr` doit appeler
  `cite_datasets()` qui lit MANIFEST.json et injecte les attributions
  → tests unitaires Rust (`cargo nextest`) vérifient présence
  obligatoire des deux citations dans `dds.xml.bytes()`.
- Spec Playwright `terroir-dds-generation-and-submission.spec.ts`
  (cf. ULTRAPLAN-TERROIR §6 P1.8) assert que le PDF DDS contient
  les chaînes "Hansen" et "JRC TMF" en footer.
- Revue trimestrielle : LIONEL TRAORE valide qu'aucun nouveau dataset
  externe n'a été ajouté sans entrée dans ce document.

---

*Dernière mise à jour : 2026-04-30 — TERROIR P1.C (Hansen + JRC mirrors).*
