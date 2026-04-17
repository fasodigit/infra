# SPEC-OUTBOX-RELAY v3.1 — FASO DIGITALISATION

**Version** : 3.1 (souverain)
**Statut** : Normative
**Propriétaire** : Équipe Plateforme — Cellule Souveraineté Numérique
**Dernière révision** : 2026-04-16

---

## 1. Résumé exécutif

`outbox-relay` est un service Rust dédié, coopérant avec chaque base YugabyteDB métier (ÉTAT-CIVIL, HOSPITAL, E-TICKET, VOUCHERS, SOGESY, E-SCHOOL, ALT-MISSION, FASO-KALAN), dont le rôle unique est de propager vers **KAYA Streams** (RESP3+) puis **Redpanda** (RAFT RF=3) tout événement métier qui a été commité atomiquement dans la table `outbox` par le service écrivain (typiquement `ingestion-rs`). Il garantit la cohérence **état → flux → journal légal** sans fenêtre de perte.

## 2. Problème adressé

### 2.1 Fenêtre silencieuse commit → XADD

Un service métier qui effectue :

```
1. COMMIT YugabyteDB (dossier créé)
2. XADD KAYA stream (notification temps réel)
3. PRODUCE Redpanda (journal légal opposable)
```

est vulnérable à tout crash entre (1) et (2) : la base est cohérente, mais aucun consumer n'a reçu l'événement. Le journal légal Redpanda, vérité opposable de FASO DIGITALISATION, est **silencieusement incomplet**. Aucun rattrapage n'est possible sans audit manuel.

### 2.2 Solution : transaction unique état+événement

Le pattern **Transactional Outbox** déplace la responsabilité : l'écrivain insère, dans la **même transaction ACID YugabyteDB**, la ligne métier ET une ligne dans `outbox`. Le relay lit ensuite `outbox` et publie. Tant que la transaction YugabyteDB est commitée, l'événement sera publié — au pire, avec un léger retard.

```sql
BEGIN;
INSERT INTO dossiers (id, nip, nom, prenoms, ...)
VALUES ('...', '...', '...', ...);

INSERT INTO outbox (
    id, aggregate_id, aggregate_type, event_type,
    payload, status, idempotency_key, partition_key, created_at
) VALUES (
    gen_random_uuid(),
    :dossier_id,
    'etat_civil.dossier',
    'etat_civil.dossier.created.v1',
    :avro_payload,
    'PENDING',
    gen_random_uuid(),
    :dossier_id::text,
    NOW()
);
COMMIT;
```

## 3. Architecture

### 3.1 Topologie

```
┌──────────────────────┐         ┌──────────────────────┐
│   ingestion-rs       │         │  services Java       │
│   (Rust, writer)     │ ◄─────► │  (GraphQL + gRPC)    │
└──────────┬───────────┘         └──────────┬───────────┘
           │    BEGIN/INSERT x2/COMMIT     │
           ▼                                ▼
        ┌─────────────────────────────────────┐
        │   YugabyteDB (table outbox)         │
        │   status = PENDING                  │
        └────────────────┬────────────────────┘
                         │ SELECT FOR UPDATE SKIP LOCKED
                         ▼
        ┌─────────────────────────────────────┐
        │   outbox-relay (Rust, 3 workers)    │
        │   2 instances HA                    │
        └──────┬──────────────────┬───────────┘
               │ 1. XADD          │ 2. PRODUCE
               ▼                  ▼
         ┌──────────┐        ┌──────────┐
         │  KAYA    │        │ Redpanda │
         │ Streams  │        │ RF=3     │
         └──────────┘        └──────────┘
```

### 3.2 Responsabilités

| Composant         | Responsabilité                                                      |
|-------------------|---------------------------------------------------------------------|
| `ingestion-rs`    | Seul writer autorisé sur la table `outbox` (user `outbox_writer`)   |
| services Java     | Envoient les payloads vers `ingestion-rs` via gRPC (Protobuf/Avro)  |
| YugabyteDB        | Stockage durable ACID de `outbox` + données métier                  |
| `outbox-relay`    | Lecture, publication ordonnée KAYA→Redpanda, marquage SENT          |
| KAYA Streams      | Hot path intra-service (consumers temps réel, < 1ms)                |
| Redpanda          | Journal légal opposable RAFT RF=3, rétention longue durée           |

## 4. Dimensionnement

- **Cible débit** : 1 000 événements/s soutenu (10× le trafic de pointe mesuré en avril 2026 sur ÉTAT-CIVIL : 100 evts/s pic).
- **Workers par instance** : 3 (tokio tasks concurrentes).
- **Instances HA** : 2 (active/active, stateless, déployées sur AZ différentes).
- **Total workers actifs** : 6.
- **Throughput cible par worker** : 200 evts/s → 1 200 evts/s agrégé.
- **Latence p99 commit→PRODUCE** : < 50 ms.
- **Latence p99 commit→XADD** : < 10 ms.

## 5. Partitionnement et ordering

### 5.1 Garantie

Ordering **strict par `aggregate_id`** (un dossier d'état civil, un patient, un ticket) mais pas d'ordering global inter-agrégats.

### 5.2 Algorithme

Chaque worker `i` (0 ≤ i < N=6) ne traite que les lignes telles que :

```
murmur3_128(aggregate_id) mod N == i
```

Implémentation Rust : `mur3::murmurhash3_x64_128` (alternative SHA-256 tronquée 64 bits acceptée si dépendance déjà présente).

Calcul côté SQL pour la clause WHERE : on persiste `partition_shard SMALLINT` calculée à l'INSERT, ce qui évite l'appel hash en SQL. Sinon, on utilise une fonction SQL immutable :

```sql
SELECT * FROM outbox
WHERE status = 'PENDING'
  AND partition_shard = :worker_id
ORDER BY created_at
LIMIT 100
FOR UPDATE SKIP LOCKED;
```

### 5.3 Pourquoi ça garantit l'ordre

Deux événements sur le même `aggregate_id` → même `partition_shard` → même worker (sous réserve du nombre de workers constant) → traités séquentiellement dans l'ordre `created_at`.

Si le nombre de workers change (scale-out/scale-in), un bref ré-équilibrage se produit. L'idempotence consumer-side (via `idempotency_key`) absorbe d'éventuels doublons, et `SKIP LOCKED` empêche deux workers de traiter la même ligne.

## 6. Isolation transactionnelle

**READ COMMITTED** suffit. L'ordering est garanti par le partitionnement, pas par le niveau d'isolation. SERIALIZABLE coûterait cher sans bénéfice fonctionnel.

`FOR UPDATE SKIP LOCKED` sous READ COMMITTED :
- lock en ligne sur la sélection ;
- les autres workers sautent les lignes verrouillées ;
- le lock tient jusqu'à la fin de la transaction du worker (update status='SENT' ou rollback).

## 7. Séquence de publication

Ordre **strict et non négociable** :

### Étape 1 — XADD KAYA (hot path)

```
XADD <stream_key> NOMKSTREAM * event_type <...> payload <...> idempotency_key <...>
```

- `<stream_key>` = `<module>.events.<aggregate_type>` (ex : `etat-civil.events.dossier`).
- Timeout : 500 ms. Échec → on passe en mode dégradé KAYA (étape 1 ignorée, étape 2 obligatoire).
- Latence cible : < 1 ms.

### Étape 2 — PRODUCE Redpanda (durabilité légale)

```
topic: <module>.events.v1
key:   <aggregate_id>  // partition sticky per aggregate
headers:
    event_type: <...>
    idempotency_key: <uuid>
    schema_id: <n>      // Schema Registry
value: <avro_payload>
acks=all, enable.idempotence=true, max.in.flight=1
```

- Timeout : 15 s. Échec → retry avec backoff (§9), puis DEAD_LETTER.
- Latence cible : 2-15 ms.

### Étape 3 — UPDATE outbox

```sql
UPDATE outbox
SET status = 'SENT', updated_at = NOW()
WHERE id = :event_id;
COMMIT;
```

Si le crash survient entre étape 2 et étape 3 : au redémarrage, le relay re-publiera — l'idempotence consumer-side (via `idempotency_key`) absorbe le doublon.

## 8. Idempotence

- `idempotency_key` UUID unique par ligne `outbox`, généré côté writer.
- Transmis en **header Kafka** Redpanda ET en champ du payload KAYA Stream.
- Les consumers maintiennent une table `processed_keys (key UUID PRIMARY KEY, processed_at TIMESTAMPTZ)` avec TTL 30 jours, et rejettent les doublons.
- Clé de partition Redpanda = `aggregate_id` → garantie de livraison in-order sur la même partition.

## 9. Retry et backoff

Backoff exponentiel avec jitter ±20 % :

| Tentative | Délai base |
|-----------|------------|
| 1         | 200 ms     |
| 2         | 400 ms     |
| 3         | 800 ms     |
| 4         | 1 600 ms   |
| 5         | 3 200 ms   |
| 6+        | `status='DEAD_LETTER'` + alerte Prometheus `OutboxDeadLetter` |

`retry_count` persisté en base, `error_reason` stocké en clair (512 caractères max).

## 10. Rétention et partitionnement temporel

Table `outbox` partitionnée par jour :

```sql
CREATE TABLE outbox (...) PARTITION BY RANGE (created_at);
CREATE TABLE outbox_2026_04_16 PARTITION OF outbox
    FOR VALUES FROM ('2026-04-16') TO ('2026-04-17');
-- etc. 7 partitions pré-créées en rolling
```

### Politique

- Partitions dont **toutes les lignes sont en `SENT`** et dont la date est > 7 jours → `DROP PARTITION`.
- Lignes `DEAD_LETTER` : conservées **90 jours** en base, puis archivées dans le topic Redpanda `outbox.dead-letter.v1` (compaction, rétention 10 ans pour compliance).
- Un cron quotidien `outbox-gc` (Rust binaire séparé ou script SQL programmé) exécute la rotation à 02:00 Africa/Ouagadougou.

## 11. Index

### Index partiel pour la boucle de lecture (réduit 10 000× la taille de l'index) :

```sql
CREATE INDEX idx_outbox_pending
    ON outbox (partition_shard, created_at)
    WHERE status = 'PENDING';
```

### Index auxiliaire pour debug / replay per-aggregate :

```sql
CREATE INDEX idx_outbox_aggregate
    ON outbox (aggregate_id, created_at);
```

### Unique sur idempotency_key (déjà sur la PK secondaire) :

```sql
CREATE UNIQUE INDEX idx_outbox_idempotency ON outbox (idempotency_key);
```

## 12. Observabilité

### 12.1 Métriques Prometheus (exposées sur `:9090/metrics`)

| Nom                                   | Type      | Description                                  |
|---------------------------------------|-----------|----------------------------------------------|
| `outbox_relay_pending_count`          | Gauge     | Nb lignes PENDING total (par module)         |
| `outbox_relay_dead_letter_count`      | Gauge     | Nb lignes DEAD_LETTER                        |
| `outbox_relay_lag_seconds`            | Histogram | Délai `NOW() - created_at` à l'instant UPDATE|
| `outbox_relay_publish_duration_ms`    | Histogram | Durée étapes 1+2 (XADD + PRODUCE)            |
| `outbox_relay_xadd_duration_ms`       | Histogram | Étape 1 seule                                |
| `outbox_relay_produce_duration_ms`    | Histogram | Étape 2 seule                                |
| `outbox_relay_retry_count_total`      | Counter   | Nb retries cumulés                           |
| `outbox_relay_worker_up`              | Gauge     | 1 si worker vivant, 0 sinon                  |

### 12.2 Logs structurés (tracing JSON)

Chaque publication émet un span `outbox.publish` avec `event_id`, `aggregate_id`, `event_type`, `retry_count`, `xadd_ms`, `produce_ms`, résultat.

### 12.3 Traces OpenTelemetry

Propagation du `traceparent` depuis le payload (header `trace_id` injecté par le writer) vers les spans KAYA et Redpanda.

## 13. Modes dégradés

### 13.1 KAYA indisponible

- Étape 1 échoue → log WARN + incrément `outbox_relay_xadd_failures_total`.
- On **ne bloque pas** l'étape 2 : Redpanda est la vérité légale, KAYA n'est qu'un hot path.
- Au prochain retry (après reprise KAYA), on ne re-XADD pas (l'événement aura déjà été produit sur Redpanda) ; les consumers KAYA rattrapent via le topic Redpanda si besoin.

### 13.2 Redpanda indisponible

- Étape 2 échoue → **circuit breaker ouvre** après 5 échecs consécutifs sur 30 s.
- Les workers en CB ouvert pausent 30 s puis testent un ping (`PRODUCE` sur topic `__ping`).
- Backlog croît en `PENDING`. Alerte `OutboxBacklogHigh` à 100 lignes.
- Aucune donnée perdue : YugabyteDB conserve tout.

### 13.3 YugabyteDB indisponible

- Les workers tombent en boucle de reconnect, exposent `outbox_relay_worker_up=0`.
- L'alerte vient des writers en amont ; outbox-relay n'a rien à faire.

## 14. Tests chaos obligatoires

À exécuter sur environnement de préprod, fréquence mensuelle :

1. **Kill Redpanda 10 min** → vérifier backlog, reprise, aucune perte (compter messages in vs out).
2. **Kill worker Outbox à chaud** (SIGKILL) → vérifier qu'un autre worker reprend les lignes lockées après expiration du lock YugabyteDB.
3. **Corruption réseau** (tc netem, 30 % de pertes, 200 ms latence) entre relay et Redpanda → vérifier retry + circuit breaker.
4. **Crash entre XADD et PRODUCE** (fault injection dans code Rust, feature flag `chaos.kill-after-xadd`) → vérifier que le doublon KAYA est absorbé par idempotency consumer-side.
5. **Split-brain YugabyteDB** → vérifier que les writers n'insèrent plus (pas de rôle du relay ici, mais test d'intégration).

## 15. Sécurité

### 15.1 Rôles YugabyteDB

| Rôle                | Privilèges                                                 |
|---------------------|------------------------------------------------------------|
| `outbox_writer`     | `INSERT` sur `outbox` (utilisé par `ingestion-rs`)         |
| `outbox_reader_relay` | `SELECT`, `UPDATE(status,retry_count,error_reason,updated_at)` sur `outbox` |
| `outbox_admin`      | `ALL` (DDL, DROP partitions, backups)                      |

Pas de `DELETE` direct : rotation par `DROP PARTITION` seulement. Pas d'`UPDATE` sur `payload` (immutabilité événementielle).

### 15.2 Authentification

- **outbox-relay** ↔ YugabyteDB : mTLS + password dérivé SPIFFE SVID court-vie (1h).
- **outbox-relay** ↔ Redpanda : SASL/SCRAM-SHA-512 + mTLS, ACL `produce` sur topics matchant le regex `^[a-z\-]+\.events\.v[0-9]+$` uniquement.
- **outbox-relay** ↔ KAYA : mTLS SPIFFE SVID, ACL KAYA restreint aux commandes `XADD`, `PING`.

### 15.3 SPIRE

- SVID déployé via SPIRE Agent sideсar Kubernetes.
- Rotation toutes les heures, sans downtime.
- Identifiant SVID : `spiffe://faso.bf/services/outbox-relay/<module>`.

## 16. Interactions avec le reste de l'écosystème

- **`ingestion-rs`** (seul writer Outbox) : reçoit gRPC des services Java, valide, hash, insère atomiquement.
- **Services Java** (Spring Boot 21) : ne touchent jamais directement à `outbox` ; ils appellent `ingestion-rs` en gRPC.
- **ARMAGEDDON** (Pingora gateway) : route le trafic entrant mais n'intervient pas sur le chemin Outbox.
- **Schema Registry FASO** : vérifie que `payload` est un Avro valide et enregistré avant publication. ID de schéma injecté en header Redpanda.

## 17. Ce qui est **hors scope**

- Le schéma Avro des événements (→ `schema-registry/`).
- La réplication cross-région YugabyteDB (→ `rpo-rto/`).
- L'orchestration K8s / Helm charts (→ `INFRA/k8s/`).

---

**Fin de SPEC-OUTBOX-RELAY v3.1**
