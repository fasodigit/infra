# ADR-0001 — MinIO retenu comme object store souverain (vs Ceph)

* Statut : ACCEPTÉ
* Date : 2026-04-27
* Décideurs : LIONEL TRAORE, équipe FASO DIGITALISATION
* Couvre : observabilité (logs Loki 5 ans, traces Tempo 90 j, métriques
  Thanos 13 mois), backups Postgres/KAYA/Vault/Consul, futurs payloads
  PII chiffrés (audit Loi 010-2004).

## Contexte

Les configurations historiques (`loki.yaml`, `tempo.yaml`, `thanos-sidecar.yaml`)
pointaient sur `s3.gra.io.cloud.ovh.net` (Gravelines, France). Ce choix viole
deux exigences :

1. **Souveraineté** (CLAUDE.md §3) — les données opérationnelles et
   réglementaires d'une administration burkinabè ne peuvent pas résider sur
   un cloud commercial étranger.
2. **Loi 010-2004 sur la protection des données personnelles** — les logs
   d'audit avec rétention 5 ans contiennent des PII (acteurs, IP,
   user-agent) qui doivent rester sous juridiction nationale.

Trois alternatives évaluées : MinIO, Ceph (RGW), SeaweedFS.

## Décision

**MinIO** est retenu comme object store souverain unique pour FASO.

Déploiement :
- Dev : single-node container (`podman-compose.observability.yml`).
- Prod : opérateur MinIO sur Kubernetes, 4 nœuds, erasure-coding EC:4+2,
  rotation TLS via cert-manager + SPIFFE.
- Sauvegardes croisées : réplication MinIO active-passive sur un second
  cluster dans une autre zone Burkina.

## Conséquences positives

- **API 100 % S3-compatible** — clients Loki/Tempo/Thanos/Velero/restic
  fonctionnent sans changement.
- **Binaire unique en Go** — pas de daemon ZooKeeper/etcd, pas de RADOS,
  démarrage < 5 s, footprint < 200 MB.
- **Console web intégrée** — pas besoin de Ceph Dashboard ou outil tiers.
- **Mode BYOC** — peut tourner sur du matériel "white-box" disponible
  localement à Ouagadougou (pas de dépendance fournisseur cloud).
- **Licence AGPLv3** — compatible avec la licence du projet FASO.

## Conséquences négatives / risques

- MinIO < Ceph en multi-tenancy fine et en stockage bloc/fichier (mais
  FASO n'a besoin que d'object). Acceptable.
- Erasure-coding MinIO moins flexible que CRUSH map de Ceph. Acceptable
  pour notre échelle (< 100 To horizon 5 ans).
- Performance write fan-out moindre que Ceph distribué massivement. Pas
  un blocage : workload FASO ≈ 1 GB/jour de logs en pic.

## Pourquoi pas Ceph

- Complexité opérationnelle prohibitive (3 daemons : MON, MGR, OSD ;
  CRUSH maps ; rebalancing manuel) pour notre équipe (2-3 SRE).
- Footprint : 8+ Go RAM minimum par OSD vs MinIO 200 MB par instance.
- Time-to-deploy : 3-5 jours d'intégration vs 1 jour MinIO.

## Pourquoi pas SeaweedFS

- Plus jeune (moins de retours d'expérience prod en Afrique de l'Ouest).
- Maturité S3 layer encore inférieure (gaps documentés : ACL, multipart
  edge cases). Risque pour intégration Loki/Tempo.

## Alignement implémentation

- `observability/grafana/config/loki.yaml` — endpoint paramétré
  `${S3_ENDPOINT:-minio.faso-observability.svc:9000}`.
- `observability/grafana/config/tempo.yaml` — idem.
- `observability/thanos/thanos-sidecar.yaml` — idem.
- `observability/grafana/podman-compose.observability.yml` — service
  `minio` + initialiseur `minio-init` qui crée les 3 buckets et leurs
  rules ILM (5 ans logs, 90 j traces, 395 j métriques).
- Credentials sourcés depuis Vault path `faso/observability/s3` en prod
  (Vault Agent template) ; defaults dev seulement.

## Références

- CLAUDE.md §3 — règle de souveraineté absolue (KAYA, ARMAGEDDON,
  xds-controller).
- Loi 010-2004 du Burkina Faso sur la protection des données.
- Audit du PR 2026-04-27 — finding CRITICAL #5.
