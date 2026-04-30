<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# RUNBOOK — Geo Mirrors (Hansen GFC + JRC TMF) — TERROIR P1.C

| Champ | Valeur |
|---|---|
| Statut | Actif |
| Date | 2026-04-30 |
| Owner | LIONEL TRAORE — TERROIR P1 lead, DevOps SRE on-call |
| Lié à | `INFRA/scripts/sync-hansen-gfc.sh`, `INFRA/scripts/sync-jrc-tmf.sh`, `INFRA/observability/cronjobs/geo-mirror-sync.yaml`, `INFRA/terroir/docs/LICENSES-GEO.md` |

## 1. Objet

Mode opératoire pour mirrorer en interne MinIO les datasets externes
utilisés par `terroir-eudr` (validation EUDR offline-first) :

- **Hansen Global Forest Change v1.11** (UMD/GLAD) — tuiles GeoTIFF.
- **JRC Tropical Moist Forest v1_2024** (Commission UE) — tuiles
  GeoTIFF par bloc continental.

Permet à `terroir-eudr` de fonctionner sans dépendance runtime sur
USGS/UMD ou EU JRC (souveraineté + résilience réseau Sahel).

## 2. Pré-requis

### 2.1 Stack

- MinIO `:9201` (S3 API) up — déployé via
  `INFRA/observability/grafana/podman-compose.observability.yml`.
- Bucket `geo-mirror` (créé automatiquement par les scripts si absent).
- Connectivité sortante HTTPS vers :
  - `storage.googleapis.com` (Hansen GFC bucket public)
  - `ies-ows.jrc.ec.europa.eu` (JRC IFORCE portal)

### 2.2 Outils CLI

- `mc` (MinIO Client) installé localement
  → https://min.io/docs/minio/linux/reference/minio-mc.html

  **OU** fallback automatique : si `mc` absent du PATH, les scripts
  utilisent `podman exec faso-minio mc ...` (le conteneur MinIO embarque
  son propre `mc`).

- `curl`, `jq` (présents par défaut sur la plupart des distros).

### 2.3 Credentials MinIO

Pour le mirror (write), credentials root MinIO suffisent en dev :

```bash
export MINIO_ACCESS_KEY="${S3_ACCESS_KEY:-faso-dev-access-key}"
export MINIO_SECRET_KEY="${S3_SECRET_KEY:-faso-dev-secret-key-change-me-32c}"
```

En prod : credentials dédiés `geo-mirror-writer` provisionnés via
Vault (`faso/minio/geo-mirror-writer`). Pour le runtime `terroir-eudr`
(read-only), utiliser `faso/minio/geo-mirror-readonly` (cf. §6 ci-dessous).

## 3. Lancement manuel

### 3.1 Hansen GFC

```bash
bash INFRA/scripts/sync-hansen-gfc.sh
```

Téléchargements estimés :
- 4 tuiles BF × 3 layers (lossyear, treecover2000, datamask)
- ~400 MB par tuile×layer compressé GeoTIFF → **~4.8 GB total**
- Durée typique : 30–60 min sur fibre, 2–4 h en EDGE/3G

### 3.2 JRC TMF

```bash
bash INFRA/scripts/sync-jrc-tmf.sh
```

Téléchargements estimés :
- 3 tuiles Afrique de l'Ouest (BFA, CIV, GHA)
- ~100 MB par tuile → **~300 MB total**
- Durée typique : 5–15 min sur fibre

### 3.3 Smoke test (validation flow end-to-end)

```bash
SMOKE_TEST=1 bash INFRA/scripts/sync-hansen-gfc.sh
SMOKE_TEST=1 bash INFRA/scripts/sync-jrc-tmf.sh
```

→ télécharge 1 seul tile pour valider chemin réseau + permissions MinIO.

### 3.4 Dry-run (audit sans I/O)

```bash
DRY_RUN=1 bash INFRA/scripts/sync-hansen-gfc.sh
DRY_RUN=1 bash INFRA/scripts/sync-jrc-tmf.sh
```

→ logge les tuiles qui seraient téléchargées sans écrire dans MinIO.

## 4. Automatisation (CronJob K8s)

`INFRA/observability/cronjobs/geo-mirror-sync.yaml` :
- **Schedule** : samedi 02:00 UTC (hebdo).
- **Concurrency** : `Forbid` — pas d'overlap si un run dépasse.
- **Timeout** : 4 h max par run.
- **Backoff** : 2 retries, history 3 succès / 5 échecs.

Apply :

```bash
kubectl apply -f INFRA/observability/cronjobs/geo-mirror-sync.yaml
```

Vérification :

```bash
kubectl -n faso-terroir get cronjob faso-geo-mirror-sync
kubectl -n faso-terroir get jobs --selector=app.kubernetes.io/name=geo-mirror-sync
kubectl -n faso-terroir logs -l app.kubernetes.io/name=geo-mirror-sync --tail=200
```

## 5. Refresh policy & versioning {#refresh-policy}

### 5.1 Principe

**Versions figées** : v1.11 Hansen, v1_2024 JRC. Aucune bascule auto.
La sync hebdo re-vérifie l'existence des fichiers (idempotent : skip si
déjà présent dans MinIO).

### 5.2 Détection drift

Le CronJob expose `geo_mirror_version_drift_total{dataset="..."}` au
Prometheus Pushgateway si le sync échoue ou détecte une nouvelle version
upstream.

**Alerte Prometheus** (à ajouter dans `INFRA/observability/alertmanager/rules/`) :

```yaml
- alert: GeoMirrorVersionDrift
  expr: increase(geo_mirror_version_drift_total[7d]) > 0
  for: 1h
  labels:
    severity: warning
    component: terroir-eudr
  annotations:
    summary: "Geo-mirror dataset drift detected for {{ $labels.dataset }}"
    runbook_url: "https://github.com/faso/INFRA/blob/main/INFRA/terroir/docs/RUNBOOK-GEO-MIRRORS.md#refresh-policy"
```

### 5.3 Procédure de bump version

1. **Validation business** : SME EUDR + LIONEL TRAORE valident la nouvelle
   version (changelog upstream, impact sur DDS existants).
2. **Bump ConfigMap** :
   ```bash
   kubectl -n faso-terroir edit configmap faso-geo-mirror-config
   # hansen-version: v1.12
   ```
3. **Run manuel sur la nouvelle version** :
   ```bash
   HANSEN_VERSION=v1.12 bash INFRA/scripts/sync-hansen-gfc.sh
   ```
4. **Re-validation différentielle** : déclencher batch
   `terroir-eudr-validator` sur les DDS existants, comparer
   `Outcome.score` ancien vs nouveau, alerter si delta > 5%.
5. **Communication** : notifier-ms publie événement
   `terroir.dataset.version.bumped` → email exportateurs concernés
   (révision DDS si nécessaire).

## 6. Vault paths {#vault-paths}

### 6.1 Lecture (terroir-eudr runtime)

Path : `faso/minio/geo-mirror-readonly`
Champs :
- `access_key` : `terroir-eudr-readonly`
- `secret_key` : généré 32 bytes hex, rotation 90j

Seed via `INFRA/vault/scripts/seed-admin-secrets.sh` (cf. §7 ci-dessous).

### 6.2 Écriture (CronJob sync)

Path : `faso/minio/geo-mirror-writer`
Champs :
- `access_key` : `geo-mirror-writer`
- `secret_key` : généré 32 bytes hex, rotation 30j

Provisioning policy MinIO :

```bash
mc admin policy create faso geo-mirror-writer - <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {"Effect": "Allow", "Action": ["s3:*"], "Resource": ["arn:aws:s3:::geo-mirror/*"]}
  ]
}
EOF
mc admin policy create faso geo-mirror-readonly - <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {"Effect": "Allow", "Action": ["s3:GetObject", "s3:ListBucket"], "Resource": ["arn:aws:s3:::geo-mirror/*", "arn:aws:s3:::geo-mirror"]}
  ]
}
EOF
```

## 7. Lecture côté `terroir-eudr` (P1.B)

Crate Rust `terroir-eudr-validator` lit MinIO via `aws-sdk-s3`
(préféré) ou `rusoto_s3`. Configuration :

```rust
let creds = vault_client.kv_get("faso/minio/geo-mirror-readonly").await?;
let s3 = aws_sdk_s3::Client::from_conf(
    aws_sdk_s3::config::Builder::new()
        .endpoint_url(env::var("MINIO_ENDPOINT")?)  // "http://faso-minio.faso-observability.svc.cluster.local:9000" en K8s
        .credentials_provider(...)
        .region(Region::new("bf-ouaga-1"))
        .force_path_style(true)  // OBLIGATOIRE MinIO
        .build()
);
let resp = s3.get_object()
    .bucket("geo-mirror")
    .key("hansen-gfc/v1.11/Hansen_GFC-2024-v1.11_lossyear_10N_010W.tif")
    .send()
    .await?;
```

**Cache local** : `terroir-eudr` cache les tuiles décodées dans
`/var/cache/terroir-eudr/tiles/` (LRU disk 5 GB max, eviction 30j) ET
les résultats de validation par polygone dans KAYA
`terroir:eudr:result:{polygon_hash}` TTL 30j (cf. ULTRAPLAN §6 P1.3).

## 8. Troubleshooting

### 8.1 `mc alias set` échoue

→ MinIO injoignable. Vérifier :
```bash
curl -fsSL http://localhost:9201/minio/health/live
podman ps --filter name=faso-minio
```

### 8.2 JRC TMF download retourne 404 {#jrc-manual}

L'endpoint IFORCE peut changer. **Fallback manuel** :

1. Aller sur https://forobs.jrc.ec.europa.eu/TMF/data/
2. Télécharger les fichiers `AnnualChange_BFA.tif`, `AnnualChange_CIV.tif`,
   `AnnualChange_GHA.tif`.
3. Upload manuel :
   ```bash
   mc alias set faso http://localhost:9201 \
       "$MINIO_ACCESS_KEY" "$MINIO_SECRET_KEY"
   mc cp AnnualChange_*.tif faso/geo-mirror/jrc-tmf/v1_2024/
   ```
4. Régénérer le MANIFEST.json :
   ```bash
   bash INFRA/scripts/sync-jrc-tmf.sh
   # → re-met juste à jour le MANIFEST (fichiers déjà présents skipped)
   ```

### 8.3 Hansen GFC retourne 403/410

GLAD a peut-être bumpé `GFC-${YEAR}-v1.x` vers une version >. Vérifier :

```bash
curl -fsSI "https://storage.googleapis.com/earthenginepartners-hansen/GFC-2024-v1.11/Hansen_GFC-2024-v1.11_datamask_10N_010W.tif"
```

Si 410 → bump `HANSEN_VERSION` (cf. §5.3).

### 8.4 Espace disque MinIO saturé

```bash
mc du faso/geo-mirror
mc admin info faso
```

Si proche du seuil → augmenter PVC MinIO ou activer ILM rule
`expire-days 1095` (3 ans) sur les versions obsolètes.

## 9. Estimation taille mirror (référence)

| Dataset | Couverture P1 | Taille | Croissance an. |
|---|---|---|---|
| Hansen GFC v1.11 | 4 tiles × 3 layers | ~4.8 GB | +20% (nouveaux tiles si extension régionale) |
| JRC TMF v1_2024 | 3 tiles AfO | ~300 MB | +5% |
| **Total P1** | — | **~5.1 GB** | — |
| Total P6 (Afrique de l'Ouest complète) | — | ~25 GB | — |

PVC MinIO recommandé : **50 GB** dès P1 (marge x10).

## 10. Liens

- ULTRAPLAN TERROIR : `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §6 P1.4
- Spike validateur EUDR : `INFRA/terroir/docs/eudr-validator-spike.md`
- Licences geo : `INFRA/terroir/docs/LICENSES-GEO.md`
- Compose MinIO : `INFRA/observability/grafana/podman-compose.observability.yml`
- ADR DDS schema : `INFRA/terroir/docs/adr/ADR-004-eudr-dds-schema.md`

---

*Dernière mise à jour : 2026-04-30 — TERROIR P1.C.*
