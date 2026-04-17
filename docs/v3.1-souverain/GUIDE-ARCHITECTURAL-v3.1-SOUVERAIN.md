# GUIDE ARCHITECTURAL v3.1 — ÉCOSYSTÈME SOUVERAIN FASO DIGITALISATION

**Version** : 3.1-SOUVERAIN
**Date** : 2026-04-16
**Portée** : 7 plateformes, 107 microservices, 8 sous-projets
**Statut** : contrat d'exploitation opposable — à opposer à tout exploitant du SI FASO DIGITALISATION
**Auteur** : Direction Technique FASO DIGITALISATION
**Successeur de** : GUIDE-ARCHITECTURAL-v3.0.md (dépublié)

---

> ### REGLE D'OR (encadré de tête — non négociable)
>
> **KAYA** est la source de vérité **opérationnelle** du hot path (RPO contractualisé ≤ 1 s, latence P99 intra-DC ≤ 2 ms).
> **Redpanda** est la source de vérité **légale opposable juridiquement** (RF=3 RAFT, RPO = 0, rétention contractuelle par topic, inaltérable).
> **YugabyteDB** est la source de vérité **durable** (ACID distribué, RPO = 0, modèle relationnel, PII chiffrées).
> **Temporal** orchestre les workflows longs, les sagas et les processus humains à délai (four-eyes, rapprochements, relances).
>
> Toute écriture transverse suit la séquence immuable :
> `(1) KAYA XADD  →  (2) Redpanda PRODUCE acks=all  →  (3) UPDATE outbox SET status=SENT`.
> Toute exception à cette règle exige un ADR signé par le Comité d'Architecture Souveraine (CAS).

---

## Sommaire

1. Architecture en 4 couches complémentaires
2. Trois niveaux de vérité
3. KAYA vs Redis — pourquoi KAYA, pas autre chose
4. Atomicité : Rhai vs MULTI/EXEC vs WATCH/CAS vs Pipeline
5. Bloom Filter KAYA — dimensionnement par projet
6. Durabilité KAYA — persistence, fsync, snapshots
7. Sécurité bout-en-bout — transport, ACL, PII, RGPD
8. Pipeline d'écriture centralisé — `ingestion-rs` seul scripteur
9. Schema Registry + Protobuf — gouvernance des topics durables
10. Bibliothèque de scripts Rhai — catalogue officiel
11. Matrice d'application par sous-projet
12. Matrice de décision Rhai / MULTI-EXEC / WATCH-CAS / Pipeline
13. Pipeline PDF asynchrone
14. Transactional Outbox opérationnel
15. DR / BCP — backups, restore, chaos engineering
16. Observabilité — métriques Prometheus et dashboards Grafana
17. Modes dégradés — runbooks par composant
18. Dimensionnement — soutenu / pic / burst
19. Licences et stratégies de sortie
20. Anti-patterns interdits

---

## 1. Architecture en 4 couches complémentaires

L'écosystème FASO DIGITALISATION repose sur une séparation stricte des préoccupations en **quatre couches** dont la combinaison garantit à la fois la **latence hot path**, la **durabilité légale**, la **vérité durable relationnelle** et l'**orchestration déterministe**.

### 1.1 Vue synoptique

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        Couche 4 — Orchestration                          │
│             TEMPORAL.IO (workflows, sagas, four-eyes, delays)            │
└───────────────────────────────┬──────────────────────────────────────────┘
                                │
┌───────────────────────────────┼──────────────────────────────────────────┐
│                               ▼                                          │
│  Couche 1 — Hot Path       Couche 2 — Journal        Couche 3 — Vérité   │
│  KAYA (Rust)               REDPANDA (RF=3 RAFT)      YUGABYTEDB (ACID)   │
│  RESP3+/gRPC 6380/6381     Schema Registry Protobuf   Relational durable │
│  Rhai, Streams, Bloom      RPO=0, rétention légale    RPO=0, PII chiffré │
└──────────────────────────────────────────────────────────────────────────┘
                                ▲
                                │ (seul scripteur : ingestion-rs)
         ┌──────────────────────┴───────────────────────┐
         │  Services Java 21 (GraalVM) + Angular 21     │
         │     BFF Next.js 16 / Bun → ARMAGEDDON        │
         └──────────────────────────────────────────────┘
```

### 1.2 Rôle de chaque couche

| Couche | Composant | Rôle contractuel | Ne peut PAS |
|-------|-----------|------------------|-------------|
| 1. Hot path | **KAYA** (Rust, 6380/6381) | Atomicité intra-DC, Streams éphémères, cache, Bloom filters, scripts Rhai atomiques, locks distribués, event-bus intra-cluster | Servir de preuve juridique, garantir une rétention > 30 jours |
| 2. Journal légal | **Redpanda** (Kafka API, RF=3) | Rétention juridique opposable, conformité, audit, long-term replay, Schema Registry | Servir de store transactionnel ACID, répondre en < 5 ms P99 |
| 3. Vérité durable | **YugabyteDB** (PostgreSQL wire) | ACID distribué, PII chiffrées, requêtes relationnelles, reporting, intégrité référentielle | Absorber le pic hot path seul (latence 10-50 ms), servir de cache |
| 4. Orchestration | **Temporal.io** | Sagas compensatoires, délais (retries exponentiels, timers longs), four-eyes, workflows humains, coordination inter-services | Stocker la vérité métier, se substituer à un bus d'événements |

### 1.3 Tableau latences / RPO / règle d'or

| Propriété | KAYA | Redpanda | YugabyteDB | Temporal |
|-----------|------|----------|------------|----------|
| Latence écriture P99 intra-DC | **≤ 2 ms** | ≤ 15 ms | ≤ 50 ms | ≤ 100 ms (tick) |
| Latence lecture P99 intra-DC | **≤ 1 ms** | ≤ 10 ms | ≤ 20 ms | N/A |
| RPO contractuel | **≤ 1 s** | **0** | **0** | 0 (sur PostgreSQL/Yugabyte backend) |
| RTO contractuel | ≤ 5 min | ≤ 10 min | ≤ 15 min | ≤ 20 min |
| Rétention | snapshots 30j | topic-scoped (90j à 10a) | illimitée | configurable (workflow retention) |
| Durabilité fsync | everysec (défaut) / always (SOGESY) | RAFT 2-of-3 | RAFT Yugabyte | idem YugabyteDB |

> **CRITIQUE.** Toute tentative d'utiliser KAYA comme source légale opposable, ou Redpanda comme store transactionnel synchrone, est un **anti-pattern contractuel** qui sera rejeté en revue d'architecture. Voir §20.

### 1.4 Règle d'or de l'écriture

Toute écriture métier **à impact légal** suit strictement :

```
1. ingestion-rs : KAYA XADD <stream> MAXLEN ~ <N> * event-json
   └── si échec → abort (pas de publication Redpanda)
2. ingestion-rs : Redpanda PRODUCE <topic> acks=all, idempotent=true
   └── si échec → retry exponentiel 200/400/800/1600/3200 ms → DLQ
3. ingestion-rs : UPDATE outbox SET status=SENT, sent_at=NOW()
   └── si échec → outbox_relay_dead_letter_count++ (alerte P1)
```

Cet ordre est **immuable**. Inverser (2) et (1) désynchronise le hot path du journal légal et ouvre une fenêtre d'inconsistance.

### 1.5 Topologie de déploiement

L'écosystème est déployé sur **deux sites physiques souverains** (DC-Ouaga-Nord et DC-Bobo-Sud) reliés par une liaison dédiée 10 Gbps avec latence RTT ≤ 5 ms.

- **Site primaire** (DC-Ouaga-Nord) : l'ensemble du chemin chaud (ARMAGEDDON, services Java, ingestion-rs, KAYA primaire, Redpanda 2 brokers, YugabyteDB 2 nœuds, Temporal, Vault primaire).
- **Site secondaire** (DC-Bobo-Sud) : KAYA réplica, Redpanda 3e broker (quorum), YugabyteDB 3e nœud (RAFT), Vault standby, MinIO miroir, ARMAGEDDON passif en hot-standby.

Tous les composants critiques sont déployés en **Kubernetes souverain** (kubeadm + Cilium CNI + MetalLB pour IP flottantes), sauf les bases de données stateful (YugabyteDB, Redpanda) en bare-metal NVMe.

### 1.6 Flux applicatif typique (séquence complète)

```
[Citoyen] → [Angular 21 SPA]
     │ HTTPS TLS 1.3
     ▼
[BFF Next.js 16 / Bun]              — session cookie HTTPOnly SameSite=Strict
     │ HTTP/3 TLS 1.3
     ▼
[ARMAGEDDON]                        — HTTP/3, JWT ES384 (SENTINEL),
     │                                WAF Coraza (AEGIS),
     │                                OPA ext_authz (ARBITER),
     │                                rate-limit (ORACLE),
     │                                routage xDS (NEXUS)
     │ gRPC mTLS SPIFFE
     ▼
[service Java 21 / GraalVM]         — DGS GraphQL, Spring Boot 3.x
     │ gRPC mTLS SPIFFE
     ▼
[ingestion-rs]                      — seul scripteur KAYA + Yugabyte
     │                                EVALSHA script Rhai
     ├─(1)→ [KAYA primary 6380]     — XADD stream, set idempotency_key
     ├─(2)→ [Redpanda acks=all]     — produce Protobuf
     └─(3)→ [Yugabyte outbox]       — UPDATE status=SENT
                                      (ordre immuable)
```

---

## 2. Trois niveaux de vérité

FASO DIGITALISATION distingue explicitement **trois niveaux de vérité** opposables en audit interne, externe, ou judiciaire.

### 2.1 Vérité opérationnelle — KAYA

- **Emplacement** : cluster KAYA (2 nœuds, ports 6380 primaire / 6381 réplica).
- **Usage** : état courant du hot path (locks, compteurs, streams éphémères, caches de projection, Bloom filters).
- **Persistence** : flags `--persistence yes --fsync everysec --snapshot 60 1000`.
- **Réplication** : async RESP3+ vers réplica, lag ≤ 1 s.
- **RPO** : **≤ 1 s** (fenêtre fsync everysec).
- **Non-opposable juridiquement**. Ne peut être produit en preuve devant un tribunal administratif.

### 2.2 Vérité légale — Redpanda

- **Emplacement** : cluster Redpanda 3 brokers, RAFT RF=3, Tiered Storage S3-compatible souverain.
- **Usage** : événements métier sérialisés en **Protobuf** (Schema Registry), topics nommés `{projet}.{aggregat}.{event}.v{N}`.
- **Durabilité** : `acks=all`, `min.insync.replicas=2`, commits RAFT.
- **RPO** : **0** (aucune perte tolérée après `acks=all` reçu).
- **Rétention** : par topic (voir §9), de 90 jours à 10 ans.
- **Opposable juridiquement**. Source de vérité produite en preuve en cas de contentieux (conformité loi burkinabè 010-2004/AN sur la protection des données à caractère personnel, art. 15 & 19).

### 2.3 Vérité durable — YugabyteDB

- **Emplacement** : cluster YugabyteDB 3 nœuds (TServer + Master), RF=3.
- **Usage** : entités relationnelles avec intégrité référentielle (tables citoyens, actes, dossiers, transactions SOGESY, dossiers médicaux).
- **Chiffrement** : LUKS au repos, TDE par colonne pour PII (AES-256-GCM, clés dans Vault Transit).
- **RPO** : **0** (RAFT synchrone).
- **Rétention** : illimitée, avec purges légales programmées par Temporal.
- **Usage principal** : lectures riches (jointures, reporting, dashboards métier, exports administratifs).

### 2.4 Matrice RPO par projet

| Sous-projet | Hot path (KAYA) | Journal (Redpanda) | Durable (Yugabyte) | RPO contractuel global |
|-------------|-----------------|--------------------|--------------------|------------------------|
| ÉTAT-CIVIL | ≤ 1 s | 0 | 0 | **0** (légal + durable) |
| HOSPITAL | ≤ 1 s | 0 | 0 | **0** |
| E-TICKET | ≤ 1 s | 0 | 0 | ≤ 1 s (perte hot acceptable) |
| VOUCHERS | ≤ 1 s | 0 | 0 | **0** |
| SOGESY | **≤ 500 ms** (fsync always) | 0 | 0 | **0** (règlementation BCEAO) |
| E-SCHOOL | ≤ 1 s | 0 | 0 | ≤ 1 s |
| ALT-MISSION | ≤ 1 s | 0 | 0 | **0** |
| FASO-KALAN | ≤ 1 s | 0 | 0 | ≤ 1 s |

### 2.5 Matrice RTO par projet

| Sous-projet | RTO hot (KAYA) | RTO journal (Redpanda) | RTO durable (Yugabyte) | RTO global contractuel |
|-------------|----------------|------------------------|------------------------|------------------------|
| ÉTAT-CIVIL | 5 min | 10 min | 15 min | **≤ 15 min** |
| HOSPITAL | 2 min | 5 min | 10 min | **≤ 10 min** (vital) |
| E-TICKET | 5 min | 10 min | 15 min | ≤ 15 min |
| VOUCHERS | 5 min | 10 min | 15 min | ≤ 15 min |
| SOGESY | 2 min | 5 min | 10 min | **≤ 10 min** (BCEAO) |
| E-SCHOOL | 15 min | 15 min | 30 min | ≤ 30 min |
| ALT-MISSION | 5 min | 10 min | 15 min | ≤ 15 min |
| FASO-KALAN | 15 min | 15 min | 30 min | ≤ 30 min |

> **RAPPEL.** Les RPO et RTO ci-dessus sont **contractuels** : tout dépassement doit faire l'objet d'un incident P1, d'un post-mortem dans les 72 h et d'une communication au CAS.

### 2.6 Règles de cohérence inter-niveaux

Les trois niveaux de vérité cohabitent mais ne remplacent jamais l'un l'autre. Les règles de cohérence sont :

- **KAYA ↔ Redpanda** : KAYA est toujours écrit *avant* Redpanda. Si Redpanda est indisponible, l'événement reste en `outbox.status = PENDING` et n'est jamais considéré comme "publié" tant qu'il n'est pas confirmé `acks=all`.
- **Redpanda ↔ Yugabyte** : Yugabyte héberge la table `outbox` qui contient **la même charge utile** que le message Redpanda. En cas de divergence détectée par `reconciliator-rs`, la **vérité légale Redpanda prévaut** car elle est inaltérable et l'outbox est mise à jour pour refléter Redpanda.
- **KAYA ↔ Yugabyte** : les projections KAYA (caches, Bloom filters, streams) sont reconstructibles à partir de Yugabyte. KAYA n'est jamais source ultime d'une donnée métier. Une perte KAYA complète (scénario catastrophique) est récupérable en 24 h par warm-up depuis Redpanda.

### 2.7 Conflits et arbitrage

En cas de conflit entre les trois niveaux (divergence détectée en audit), l'arbitrage suit l'ordre hiérarchique :

1. **Redpanda** fait foi pour l'événement (source légale opposable).
2. **Yugabyte** fait foi pour l'état final relationnel (après projection validée).
3. **KAYA** n'est jamais autoritatif en cas de conflit : sa donnée est réputée *éphémère*.

Un rapprochement automatisé (`reconciliator-rs`) s'exécute toutes les 30 minutes pour détecter les divergences et produit un rapport consigné dans `audit.reconciliation.v1`.

---

## 3. KAYA vs Redis — pourquoi KAYA

KAYA est le composant souverain qui remplace intégralement Redis/DragonflyDB dans l'écosystème. Le présent guide n'utilise **jamais** Redis ou DragonflyDB comme composant actif, sauf mention explicite d'un point historique.

### 3.1 Threading multi-shard

KAYA exploite un modèle **shared-nothing multi-shard** (1 shard = 1 thread dédié, bound to core via `proactor_threads`). Contrairement à Redis (single-threaded pour l'exécution des commandes), KAYA parallélise les commandes sur les shards en routant par **hash de clé** (xxhash64). Les transactions MULTI/EXEC qui touchent plusieurs shards utilisent un **contrôle de concurrence optimiste à verrouillage de valeurs (OCC VLL)** : pas de verrou global, abort + retry en cas de conflit.

Configuration typique :

```toml
# /etc/kaya/kaya.toml
[server]
bind = "0.0.0.0"
port = 6380
proactor_threads = 4         # 1 shard par cœur physique
io_threads = 2               # epoll workers dédiés au réseau

[persistence]
enabled = true
fsync = "everysec"           # always pour SOGESY
snapshot_interval_seconds = 60
snapshot_min_changes = 1000

[replication]
role = "primary"
replica_addrs = ["kaya-replica-1:6381"]
protocol = "resp3plus"
```

### 3.2 Rhai 5.4 — scripting natif atomique

KAYA embarque **Rhai 5.4** (langage de scripting Rust) comme moteur d'atomicité serveur-side. Rhai remplace **intégralement Lua** et offre :

- typage statique optionnel (strict mode) ;
- `rhai_auto_async = true` : parallélisation transparente des opérations I/O-bound à l'intérieur du script ;
- sandboxing par défaut (pas d'accès fichier/réseau) ;
- budget opérationnel (`max_operations = 100000`) pour prévenir les scripts runaway ;
- intégration gRPC native via `kaya.eval_sha(hash, keys[], args[])`.

Exemple minimal :

```rhai
// dedup_and_persist.rhai
let idempotency_key = ARGS[0];
let event_id = ARGS[1];
let event_payload = ARGS[2];
let stream_key = KEYS[0];
let dedup_key = KEYS[1];

if kaya.exists(dedup_key) {
    return #{ status: "DUPLICATE", event_id: event_id };
}
kaya.set_ex(dedup_key, "1", 86400);
let entry_id = kaya.xadd(stream_key, "*", "event_id", event_id, "payload", event_payload);
return #{ status: "OK", entry_id: entry_id };
```

### 3.3 MULTI / EXEC via OCC VLL

La transaction multi-commande classique (MULTI/EXEC) est supportée, mais repose sur le **Value Locking Layer (VLL)** interne : chaque commande déclare ses clés, le moteur détecte les conflits à l'EXEC et abort si une clé a été modifiée entre MULTI et EXEC. Retry côté client obligatoire — voir §4.

### 3.4 Bloom filters natifs

KAYA embarque une implémentation native de **Bloom filters** (type `BF`), sans module externe (contrairement à Redis qui nécessite RedisBloom). Commandes : `BF.RESERVE`, `BF.ADD`, `BF.EXISTS`, `BF.MADD`, `BF.MEXISTS`, `BF.INFO`. Voir §5.

### 3.5 Ce que KAYA ne fait PAS

| Fonctionnalité | Statut | Alternative |
|---------------|--------|-------------|
| Redis Functions (FUNCTION LOAD) | Non supporté | Utiliser les scripts Rhai versionnés Git |
| RedisJSON | Non supporté natif | Sérialiser côté ingestion-rs en JSON string |
| RedisSearch | Non supporté | Déléguer à YugabyteDB ou à un index dédié |
| RediSearch FT.AGGREGATE | Non supporté | Projections matérialisées via KAYA Streams |
| Pub/Sub clustering legacy | Non supporté | Utiliser KAYA Streams + consumer groups |

> **CRITIQUE.** N'essayez pas de porter mécaniquement du code Lua vers Rhai. Rhai a une sémantique différente (pas de `pcall`, pas de tables mixtes). Toute migration passe par une réécriture et une revue du CAS.

### 3.6 Protocole RESP3+ et gRPC

KAYA supporte deux protocoles :

- **RESP3+** : extension propriétaire de RESP3 (Redis), compatible avec la majorité des clients Redis modernes, avec en plus le support des **types riches** (maps, sets typés), du **push server-side** (notifications), et du **framing binaire plus compact** (−15 % bande passante vs RESP3 standard).
- **gRPC** (port 6381 optionnel) : interface protobuf dédiée, recommandée pour les services Rust internes (ingestion-rs, outbox-relay) qui bénéficient du streaming bidirectionnel et du typage fort.

```proto
// kaya.v1.proto (extrait)
service Kaya {
  rpc EvalSha (EvalShaRequest) returns (EvalShaResponse);
  rpc XAdd    (XAddRequest)    returns (XAddResponse);
  rpc XReadGroup (XReadGroupRequest) returns (stream XReadGroupEntry);
  rpc BfExists (BfExistsRequest) returns (BfExistsResponse);
  rpc BfMExists (BfMExistsRequest) returns (BfMExistsResponse);
}
```

### 3.7 Collections typées et vues matérialisées

KAYA introduit deux concepts absents de Redis :

- **Collections typées** : `HMAP<string, int64>`, `LIST<UUID>`, `SET<string>` avec validation côté serveur, refus d'insertion d'un type incorrect.
- **Vues matérialisées** : `MVIEW CREATE <name> FROM <stream> AS <rhai_projection>` définit une projection calculée en live, mise à jour à chaque XADD. Exemple : compter en temps réel les validations par tenant.

```text
kaya-cli MVIEW CREATE mv:validations:count \
  FROM stream:validations \
  AS "let t = event.tenant; let current = mview.get(t).unwrap_or(0); mview.set(t, current + 1);"
```

### 3.8 Streams KAYA — détail fonctionnel

Les **Streams KAYA** sont l'analogue des Redis Streams, mais avec quelques extensions :

- **Consumer groups** : jusqu'à 256 groupes par stream.
- **XREADGROUP BLOCK** : blocking jusqu'à `N` ms ou réception.
- **XACK** + **XPENDING** : gestion du PEL (Pending Entries List).
- **XAUTOCLAIM** : réattribution automatique des entrées abandonnées par un consumer crashé.
- **Retention MAXLEN** : bornage par longueur approximative (`MAXLEN ~ 100000`).

Exemple de consumer :

```rust
// Rust client KAYA, extrait pdf-worker-ms
let entries = kaya.xreadgroup(
    "pdf-workers",          // consumer group
    "worker-1",             // consumer name
    &[("etat-civil:pdf:requested", ">")],
    Some(10),               // count
    Some(5000),             // block ms
).await?;

for entry in entries {
    match process_pdf(&entry).await {
        Ok(_) => kaya.xack("etat-civil:pdf:requested", "pdf-workers", &entry.id).await?,
        Err(e) => { /* laisser PEL, sera repris par XAUTOCLAIM */ }
    }
}
```

### 3.9 Benchmarks internes

Résultats de bench sur plateforme de référence (Scale-a7 AMD EPYC 9005, 4 proactor_threads, NVMe, RESP3+ TLS 1.3, RF=1 pour le bench pur single-node) :

| Opération | Débit | Latence P50 | Latence P99 |
|-----------|-------|-------------|-------------|
| `SET` (10B key, 100B value) | 620 k ops/s | 0.3 ms | 1.4 ms |
| `GET` (cache hit) | 1.1 M ops/s | 0.2 ms | 0.9 ms |
| `XADD` (stream, 256B payload) | 480 k ops/s | 0.4 ms | 1.8 ms |
| `BF.EXISTS` | 950 k ops/s | 0.2 ms | 0.8 ms |
| `EVALSHA` (script 30 ops) | 180 k ops/s | 0.6 ms | 2.9 ms |

> **NOTE.** Ces chiffres sont donnés à titre indicatif. Le bench officiel, à inclure dans le dossier technique opposable, est à exécuter sur le socle de production final.

---

## 4. Atomicité : Rhai vs MULTI/EXEC vs WATCH/CAS vs Pipeline

Le choix du mécanisme d'atomicité dépend du **nombre d'opérations**, de la **logique conditionnelle** et du **volume de données échangées**.

### 4.1 Matrice de décision

| Mécanisme | Atomicité | Logique conditionnelle serveur | Retry | Quand l'utiliser | Quand l'éviter |
|-----------|-----------|---------------------------------|-------|------------------|----------------|
| **Rhai (EVALSHA)** | Oui (exécution single-shard) | Oui (complète, avec branchements) | Non nécessaire | Logique > 3 commandes, idempotence, lock acquisition, dedup, four-eyes | Opérations qui nécessitent I/O externe |
| **MULTI/EXEC** | Oui (OCC VLL) | Limitée (commandes linéaires uniquement) | Oui si conflit VLL | Séquences commandes fixes, pas de conditions dynamiques | Logique conditionnelle complexe |
| **WATCH/CAS** | Optimiste (abort si WATCH violé) | Côté client | Oui | Incréments avec vérification côté client | Haute contention (thrash) |
| **Pipeline** | Non atomique (batch réseau) | Non | Non (chaque cmd peut échouer indépendamment) | Batch d'opérations indépendantes (préchauffage cache, bulk insert) | Toute séquence qui exige cohérence |

### 4.2 Exemple — acquisition de lock distribué avec TTL

**Rhai (recommandé, 1 aller-retour, atomique)** :

```rhai
// acquire_lock.rhai
let lock_key = KEYS[0];
let owner_id = ARGS[0];
let ttl_ms = ARGS[1].parse_int();

if kaya.set_nx_px(lock_key, owner_id, ttl_ms) {
    return #{ acquired: true, owner: owner_id };
}
let current_owner = kaya.get(lock_key);
return #{ acquired: false, current_owner: current_owner };
```

**WATCH/CAS (client-side, 2-3 allers-retours, risque thrash)** :

```text
WATCH lock_key
GET lock_key                → si null, continuer ; sinon abort
MULTI
SET lock_key owner_id PX ttl
EXEC                        → si null (WATCH violé), retry
```

### 4.3 Quand mixer

Rhai peut **contenir** un MULTI/EXEC implicite (Rhai 5.4 auto-async le gère), mais on préfère la version 100 % Rhai pour la lisibilité et la testabilité. MULTI/EXEC pur reste utile pour des **séquences très simples** dans les langages client qui n'ont pas de loader Rhai mature.

### 4.4 Timeouts et budgets

- Rhai : `max_operations = 100_000`, `timeout = 50 ms` (configurable par script, signalé via `kaya.set_timeout_ms()`).
- MULTI/EXEC : pas de timeout natif, le client doit fixer un `deadline` gRPC.
- WATCH/CAS : retry maximum 3, backoff jitteré 5/15/45 ms, au-delà → erreur métier.

### 4.5 Exemple avancé — four-eyes validation

```rhai
// four_eyes_validate.rhai
// KEYS[0] = "validation:<aggregate_id>"
// ARGS[0] = approver_id
// ARGS[1] = decision ("APPROVED" | "REJECTED")
// ARGS[2] = comment (optional)

let key = KEYS[0];
let approver = ARGS[0];
let decision = ARGS[1];

// Check 1 : l'approbateur n'est pas l'auteur de la demande
let requester = kaya.hget(key, "requester");
if requester == approver {
    return #{ status: "REJECTED_SELF_APPROVAL", reason: "same as requester" };
}

// Check 2 : approbateur n'a pas déjà voté
let first_approver = kaya.hget(key, "first_approver");
if first_approver == approver {
    return #{ status: "REJECTED_DOUBLE_VOTE", reason: "already voted" };
}

if first_approver == () {
    // Premier vote
    kaya.hset_multi(key, #{
        first_approver: approver,
        first_decision: decision,
        first_ts: kaya.now_ms().to_string()
    });
    kaya.expire(key, 86400);   // 24h pour second vote
    return #{ status: "PENDING_SECOND", first_approver: approver };
}

// Second vote
kaya.hset_multi(key, #{
    second_approver: approver,
    second_decision: decision,
    second_ts: kaya.now_ms().to_string()
});

let first_decision = kaya.hget(key, "first_decision");
if first_decision == decision && decision == "APPROVED" {
    kaya.hset(key, "final", "APPROVED");
    kaya.xadd("four-eyes:events", "*",
        "aggregate_id", key, "status", "APPROVED",
        "first", first_approver, "second", approver);
    return #{ status: "APPROVED", aggregate_id: key };
}

kaya.hset(key, "final", "REJECTED");
return #{ status: "REJECTED", reason: "divergent votes" };
```

### 4.6 Gestion d'erreur et observabilité

Tous les scripts Rhai doivent :

1. **Retourner un `#{ status: "...", ... }`** (map Rhai), jamais `()` implicite.
2. **Émettre un log structuré** via `kaya.log_info/warn/error(msg)` pour chaque branchement significatif.
3. **Incrémenter un compteur** `kaya.metric_inc("script_<name>_<branch>_total")` pour alimenter Prometheus.
4. **Ne jamais panic** (Rhai 5.4 convertit les panics en erreurs moteur, mais la bonne pratique est de renvoyer un status explicite).

---

## 5. Bloom Filter KAYA — dimensionnement par projet

Les Bloom filters KAYA servent à **filtrer rapidement** les lookups négatifs (numéro de dossier inexistant, ticket déjà consommé, voucher déjà grillé) avant de frapper YugabyteDB. **Gain mesuré** : −85 % de charge lecture sur Yugabyte pour les endpoints de vérification.

### 5.1 Commandes de réservation (`BF.RESERVE`)

```shell
# ÉTAT-CIVIL : 100k dossiers attendus, 0.1 % FP
kaya-cli BF.RESERVE bf:etat-civil:dossiers 0.001 100000

# E-TICKET : 1M tickets/an, 0.01 % FP (anti-doublon critique)
kaya-cli BF.RESERVE bf:eticket:consumed 0.0001 1000000

# HOSPITAL : 500k dossiers patients, 0.01 % FP
kaya-cli BF.RESERVE bf:hospital:dossiers 0.0001 500000

# VOUCHERS : 200k codes par campagne, 0.1 % FP
kaya-cli BF.RESERVE bf:vouchers:issued 0.001 200000

# E-SCHOOL : 500k élèves, 0.1 % FP
kaya-cli BF.RESERVE bf:eschool:students 0.001 500000

# SOGESY : 1M transactions, 0.01 % FP (anti-replay)
kaya-cli BF.RESERVE bf:sogesy:tx 0.0001 1000000
```

### 5.2 Double-check contre faux positifs

Un Bloom filter peut signaler **faux positif** (dit présent alors qu'absent). Jamais de faux négatif. Le workflow opérationnel est :

```text
1. BF.EXISTS bf:<projet>:<aggregat> <key>
   └── si false    → décision rapide "absent" (safe)
   └── si true     → aller interroger YugabyteDB (double-check)
        ├── si présent  → décision "présent"
        └── si absent   → faux positif, décision "absent"
```

### 5.3 Maintenance

- **Reconstruction hebdomadaire** par `ingestion-rs` via `BF.RESERVE` sur clé temporaire + `BF.MADD` bulk, puis `RENAME` atomique.
- **Monitoring** : `BF.INFO` → `expansions`, `size`, `items_inserted`, `ratio`. Alerte si `ratio > 1.5 × configured_error_rate`.

### 5.4 Script de rebuild (exécution hebdomadaire par cron-workflow Temporal)

```rhai
// bf_rebuild.rhai
// KEYS[0] = "bf:<projet>:<aggregat>"
// KEYS[1] = "bf:<projet>:<aggregat>:rebuild"    (clé temporaire)
// ARGS[0] = error_rate
// ARGS[1] = capacity

let bf_live = KEYS[0];
let bf_tmp = KEYS[1];
let error_rate = ARGS[0].parse_float();
let capacity = ARGS[1].parse_int();

kaya.bf_reserve(bf_tmp, error_rate, capacity);
// Le reload effectif (BF.MADD en batch) est fait par ingestion-rs en Rust
// Ce script se contente de switch-over atomique après reload
if kaya.exists(bf_tmp) {
    kaya.rename(bf_tmp, bf_live);
    return #{ status: "SWAPPED" };
}
return #{ status: "NO_TMP_BUILT" };
```

### 5.5 Sémantique de cohérence Bloom ↔ Yugabyte

Le Bloom filter KAYA reflète la réalité de Yugabyte **avec un lag maximum de 1 rebuild (hebdomadaire)**. Les ajouts nouveaux sont propagés en temps réel via `BF.ADD` par `ingestion-rs` lors de chaque écriture Yugabyte. Les suppressions sont gérées par **rebuild complet** (les Bloom filters standards ne supportent pas la suppression).

Pour les cas où la suppression est fréquente (ex : consommation E-TICKET), on utilise un **Counting Bloom Filter** (extension `CBF.*` de KAYA) qui accepte `CBF.DEL` mais consomme ~2× la mémoire.

### 5.6 Dimensionnement mémoire

Formule : `m = -n * ln(p) / (ln(2))^2` bits, où `n` = capacité, `p` = error rate.

| Projet | n | p | Mémoire |
|--------|-----|-----|---------|
| ÉTAT-CIVIL | 100 000 | 0.001 | ~175 KB |
| E-TICKET | 1 000 000 | 0.0001 | ~2.3 MB |
| HOSPITAL | 500 000 | 0.0001 | ~1.15 MB |
| VOUCHERS | 200 000 | 0.001 | ~350 KB |
| E-SCHOOL | 500 000 | 0.001 | ~875 KB |
| SOGESY | 1 000 000 | 0.0001 | ~2.3 MB |
| **Total** | — | — | **~7.2 MB** |

Empreinte mémoire Bloom **négligeable** face à la capacité KAYA (typiquement 32-64 GB).

---

## 6. Durabilité KAYA — persistence, fsync, snapshots

KAYA offre trois leviers de durabilité : **AOF (append-only file)**, **snapshots périodiques**, **réplication async**. Le paramétrage par défaut privilégie la latence ; SOGESY et HOSPITAL basculent en mode renforcé.

### 6.1 Flags de persistence

```shell
kaya-server \
  --persistence yes \
  --fsync everysec \
  --snapshot "60 1000" \
  --proactor_threads=4 \
  --aof_path /var/lib/kaya/aof \
  --snapshot_path /var/lib/kaya/dump
```

Détail :

- `--persistence yes` : active AOF + snapshots (OBLIGATOIRE en production, jamais `no`).
- `--fsync <always|everysec|no>` : cadence du `fsync(2)` sur l'AOF.
- `--snapshot "60 1000"` : snapshot si ≥ 1000 changements en 60 s.
- `--proactor_threads` : nombre de shards (= cœurs CPU dédiés).

### 6.2 Comparaison fsync

| Mode fsync | Latence write P99 | RPO | Usage | Projets |
|------------|-------------------|-----|-------|---------|
| `always` | 3-5 ms | **0** (écriture synchrone) | Paiements, règlements | **SOGESY** |
| `everysec` (défaut) | **≤ 2 ms** | ≤ 1 s | Hot path général | ÉTAT-CIVIL, HOSPITAL, E-TICKET, VOUCHERS, E-SCHOOL, ALT-MISSION, FASO-KALAN |
| `no` | ≤ 1 ms | dépend OS (30 s+) | **INTERDIT en prod** | — |

> **BON USAGE.** SOGESY utilise `fsync=always` pour se conformer aux exigences BCEAO sur la non-perte d'opérations de paiement. Tous les autres projets utilisent `everysec` sauf ADR contraire.

### 6.3 Snapshots et RPO étendu

- Snapshot local (`--snapshot 60 1000`) : garde 7 derniers snapshots.
- Export vers stockage souverain (MinIO S3-compatible) toutes les **6 h**.
- Rétention snapshots externes : **30 jours**.
- Chiffrement snapshot : AES-256-GCM avec clé Vault Transit rotée mensuellement.

### 6.4 Commandes admin

```shell
kaya-cli BGSAVE                  # snapshot manuel non bloquant
kaya-cli LASTSAVE                # timestamp du dernier snapshot réussi
kaya-cli DEBUG AOF REWRITE       # compactage AOF
kaya-cli CONFIG GET persistence  # vérification runtime
```

### 6.5 Tableau durabilité / latence consolidé

| Profil | `fsync` | `snapshot` | Latence write P99 | RPO | Projets |
|--------|---------|------------|-------------------|-----|---------|
| **Conservateur** | always | 60 500 | 4 ms | **0** | SOGESY |
| **Équilibré (défaut)** | everysec | 60 1000 | 2 ms | ≤ 1 s | ÉTAT-CIVIL, HOSPITAL, VOUCHERS, ALT-MISSION |
| **Performance** | everysec | 300 5000 | 1.5 ms | ≤ 1 s | E-TICKET, E-SCHOOL, FASO-KALAN |
| **Cache pur** | no | off | 0.8 ms | ∞ | **INTERDIT pour données métier** (usage cache éphémère uniquement, données reconstructibles) |

### 6.6 Réplication primaire → réplica

```toml
[replication]
role = "primary"                          # sur kaya-primary
replica_addrs = ["kaya-replica:6381"]
protocol = "resp3plus"
mtls_cert = "/etc/kaya/tls/primary.crt"
mtls_key = "/etc/kaya/tls/primary.key"
mtls_ca = "/etc/kaya/tls/ca.crt"
replication_mode = "async"                # async par défaut (latence ≤ 1s)
backlog_size_mb = 1024                    # tampon pour reconnexion réplica
```

Le réplica se connecte via `REPLICAOF kaya-primary 6380` et reçoit le flux d'AOF. Le lag est monitoré par `kaya_replication_lag_seconds`.

Promotion du réplica en primaire (manual ou auto via `kaya-sentinel`) :

```shell
kaya-cli -h kaya-replica -p 6381 REPLICAOF NO ONE
kaya-cli -h kaya-replica -p 6381 CONFIG SET role primary
```

### 6.7 Vérification d'intégrité

Au démarrage, KAYA vérifie l'AOF par un parsing complet. Si une entrée est corrompue :

- Avec `--aof-load-truncated yes` (défaut) : tronque à la dernière entrée valide et redémarre (perte éventuelle < 1 s).
- Avec `--aof-load-truncated no` : refus de démarrer, opération manuelle nécessaire (`kaya-check-aof --fix`).

---

## 7. Sécurité bout-en-bout

### 7.1 Transport

- **Nord-Sud** : Angular → BFF Next.js (Bun) → **ARMAGEDDON** (HTTP/3, TLS 1.3, JWT authn ES384, WAF Coraza, OPA ext_authz) → services backend Java.
- **Est-Ouest** : gRPC **mTLS** avec SVID SPIRE, suites `ECDHE-ECDSA-AES256-GCM-SHA384`.
- **Services ↔ KAYA** : RESP3+/TLS 1.3, mTLS activé, SVID SPIRE présenté côté client.
- **Services ↔ Redpanda** : Kafka protocol over TLS 1.3, SASL/SCRAM ou mTLS SPIFFE.
- **Services ↔ YugabyteDB** : PostgreSQL wire over TLS 1.3, certificats Vault PKI.

### 7.2 Chiffrement au repos

| Composant | Mécanisme | Clé gérée par |
|-----------|-----------|---------------|
| KAYA AOF + snapshots | LUKS (volume) + AES-256-GCM (fichier snapshot externe) | Vault Transit |
| Redpanda segments | LUKS (volume) + TLS Tiered Storage | Vault Transit |
| YugabyteDB SSTables | LUKS (volume) + TDE colonne pour PII | Vault Transit |
| PDF pipeline outputs | AES-256-GCM | Vault Transit, clé par dossier |

### 7.3 ACL KAYA par tenant

Chaque microservice dispose d'un utilisateur KAYA dédié avec ACL minimale. Exemple ÉTAT-CIVIL :

```shell
kaya-cli ACL SETUSER etat_civil_svc on \
  ~{tenant-ec}:* \
  +@read +@write +eval +evalsha +bf.* +xadd +xreadgroup \
  -keys -flushall -flushdb -config -debug -shutdown
```

Règles universelles :

- `~{tenant-<code>}:*` : isolation stricte par préfixe de clé.
- `+eval +evalsha` : autorisé **uniquement pour `ingestion-rs`** (les autres services reçoivent seulement `+evalsha`).
- `-keys -flushall -flushdb -config -debug -shutdown` : **interdits pour tous** les services applicatifs.
- Mot de passe rotatif Vault (rotation mensuelle, propagation via SPIRE SVID + sidecar injection).

### 7.4 SPIRE / SVID pour Redpanda

Chaque workload obtient un **SVID SPIFFE** au démarrage via l'agent SPIRE local. Exemple :

```text
spiffe://faso.bf/ns/etat-civil/sa/ingestion-rs
spiffe://faso.bf/ns/etat-civil/sa/demande-ms
spiffe://faso.bf/ns/hospital/sa/ingestion-rs
```

Côté Redpanda, ACL par principal SPIFFE :

```shell
rpk security acl create \
  --allow-principal "User:spiffe://faso.bf/ns/etat-civil/sa/ingestion-rs" \
  --operation write \
  --topic "etat-civil.*"

rpk security acl create \
  --allow-principal "User:spiffe://faso.bf/ns/etat-civil/sa/demande-ms" \
  --operation read \
  --topic "etat-civil.*" \
  --group "etat-civil-demande-consumer"
```

### 7.5 Policy PII / RGPD

Conformément à la loi burkinabè 010-2004/AN et au RGPD (référentiel CNIL transposé) :

- **Dans Redpanda** : UUID opaques uniquement, **jamais de PII en clair**. Les événements contiennent `citizen_id = <uuid>` mais pas `name`, `phone`, `email`.
- **Dans YugabyteDB** : les PII sont chiffrées par colonne via TDE AES-256-GCM, clé maître dans Vault Transit, rotée tous les 6 mois.
- **Droit à l'oubli** : implémenté par workflow Temporal `citizen-erasure-v1` qui (1) chiffre avec clé jetée (crypto-shredding) dans Yugabyte, (2) publie événement `citizen.erased.v1` tombstone dans Redpanda (compaction), (3) supprime projections KAYA.
- **Audit d'accès** : tous les lookups PII sont loggés dans le topic `audit.pii-access.v1` (rétention 10 ans).

### 7.6 Chaîne JWT — ARMAGEDDON

```text
1. Citoyen → BFF Next.js : login OAuth2/OIDC (Keycloak souverain BF)
2. BFF stocke refresh_token en backend, émet access_token (JWT ES384)
   └── payload: sub, tenant, roles, exp (15 min), iat, jti
3. Appels API portent Authorization: Bearer <jwt>
4. ARMAGEDDON/SENTINEL valide la signature ES384 (clé publique via JWKS endpoint du Keycloak BF)
5. ARMAGEDDON/ARBITER appelle OPA ext_authz :
   input = { jwt_claims, method, path, headers }
   policy Rego : autorise ou refuse
6. ARMAGEDDON/AEGIS (WAF Coraza) : inspection payload, OWASP Top10
7. ARMAGEDDON/NEXUS : route vers backend (xDS cluster)
```

Clé ES384 :

- Rotée tous les **90 jours** via Vault PKI.
- Publication JWKS à l'URL `/.well-known/jwks.json` du Keycloak BF.
- Validation offline possible (cache JWKS 1h côté ARMAGEDDON).

### 7.7 Politique OPA (exemple Rego)

```rego
package faso.authz

default allow = false

# Lecture publique d'un dossier par son propriétaire
allow {
    input.method == "GET"
    input.path[0] == "etat-civil"
    input.path[1] == "dossiers"
    input.jwt_claims.sub == input.path[2]
}

# Validation d'acte : rôle officier OU administrateur tenant
allow {
    input.method == "POST"
    input.path[0] == "etat-civil"
    input.path[1] == "actes"
    input.path[3] == "validate"
    some r
    r := input.jwt_claims.roles[_]
    r == "etat-civil:officier"
}
```

### 7.8 Rotation de secrets — procédure standard

| Secret | Fréquence | Mécanisme |
|--------|-----------|-----------|
| JWT signing key (ES384) | 90 j | Vault PKI + JWKS publication |
| mTLS SVID workload | 1 h (auto) | SPIRE agent local |
| mTLS SVID long-lived (ex. Redpanda principals) | 24 h | SPIRE + node attestor Kubernetes |
| Mot de passe KAYA par service | 30 j | Vault KV v2 + sidecar consul-template |
| Clé TDE colonne Yugabyte | 6 mois | Vault Transit, rewrap sans perte |
| Clés HMAC chaîne SOGESY | 12 mois | Vault Transit dedicated keyring |
| Clé de chiffrement snapshots KAYA/Redpanda/Yugabyte | 12 mois | Vault Transit |

---

## 8. Pipeline d'écriture centralisé — `ingestion-rs` seul scripteur

### 8.1 Principe

**Seul le service `ingestion-rs` (Rust, GraalVM non applicable ici, build natif Rust stable)** a le droit d'écrire simultanément vers **KAYA** et **YugabyteDB**. Les services Java métier (demande-ms, traitement-acte-ms, etc.) n'ont **jamais** d'accès write direct à ces deux stores : ils passent par **gRPC vers `ingestion-rs`**.

Avantages :

- Point unique d'enforcement de la règle d'or (§1.4).
- Centralisation des scripts Rhai, des schemas Protobuf et de la table outbox.
- Politique de retry et backoff uniforme.
- Facilité d'audit (un seul chemin d'écriture).

### 8.2 Flux typique

```text
demande-ms (Java 21)
  │  gRPC IngestEvent(event, tenant, aggregate_id, idempotency_key)
  ▼
ingestion-rs (Rust)
  ├── 1. EVALSHA dedup_and_persist.rhai (KAYA)
  │       └── si DUPLICATE → return EventResult{status=IDEMPOTENT_HIT}
  ├── 2. INSERT outbox (Yugabyte, PENDING)
  ├── 3. KAYA XADD stream + Redpanda PRODUCE + UPDATE outbox SENT
  └── 4. return EventResult{status=OK, entry_id, offset}
```

### 8.3 Contrat gRPC `ingestion.v1.IngestionService`

```proto
syntax = "proto3";
package faso.ingestion.v1;

service IngestionService {
  rpc IngestEvent(IngestEventRequest) returns (IngestEventResponse);
  rpc IngestBatch(stream IngestEventRequest) returns (stream IngestEventResponse);
}

message IngestEventRequest {
  string tenant = 1;               // "etat-civil", "hospital", ...
  string aggregate_id = 2;         // UUID
  string idempotency_key = 3;      // UUID fourni par le client
  string event_type = 4;           // "demande.created.v1"
  bytes  event_payload = 5;        // Protobuf encoded
  int64  occurred_at_unix_ms = 6;
}

message IngestEventResponse {
  Status status = 1;               // OK | IDEMPOTENT_HIT | REJECTED
  string kaya_entry_id = 2;
  int64  redpanda_offset = 3;
  string trace_id = 4;
}
```

### 8.4 Back-pressure

`ingestion-rs` expose un **semaphore par tenant** (default 128 concurrent writes) et retourne `RESOURCE_EXHAUSTED` si dépassement. Les clients Java utilisent Resilience4j bulkhead + circuit breaker.

### 8.5 Isolation tenant dans ingestion-rs

Chaque requête `IngestEventRequest` porte un champ `tenant` qui fait référence à un **descripteur de tenant** chargé au boot :

```yaml
# ingestion-rs.tenants.yaml
tenants:
  - name: etat-civil
    kaya_stream_prefix: "etat-civil"
    kaya_acl_user: "etat_civil_svc"
    redpanda_principal: "spiffe://faso.bf/ns/etat-civil/sa/ingestion-rs"
    outbox_partition_prefix: "EC"
    max_concurrent_writes: 128
    fsync_profile: "everysec"
  - name: sogesy
    kaya_stream_prefix: "sogesy"
    kaya_acl_user: "sogesy_svc"
    redpanda_principal: "spiffe://faso.bf/ns/sogesy/sa/ingestion-rs"
    outbox_partition_prefix: "SG"
    max_concurrent_writes: 256
    fsync_profile: "always"   # BCEAO
  # ... 6 autres tenants
```

### 8.6 Garanties d'ordre

- **Intra-aggregate_id** : strict (un même agrégat transite toujours par le même worker outbox-relay via `hash(aggregate_id) mod N`).
- **Inter-aggregate_id** : aucun ordre garanti (mais ce n'est pas requis pour les cas d'usage identifiés).
- **Dans Redpanda** : partition = `hash(partition_key) mod num_partitions`, où `partition_key = aggregate_id` → ordre préservé par agrégat.

### 8.7 Idempotence

Chaque requête porte un `idempotency_key` (UUID) fourni par le client :

- **Unicité** : contrainte `UNIQUE` sur la colonne `outbox.idempotency_key`.
- **Rejeu** : si même `idempotency_key` reçu deux fois, ingestion-rs retourne `IDEMPOTENT_HIT` avec la référence de l'événement original (pas de double écriture).
- **TTL idempotence KAYA** : la clé `dedup:<tenant>:<idempotency_key>` expire après **24 h** (fenêtre suffisante pour tous les retries clients).

### 8.8 Validation des événements à l'entrée

`ingestion-rs` effectue **à la réception** :

1. **Validation sémantique** : le `event_type` doit exister dans le catalogue connu (mapping topic ← event_type).
2. **Validation Protobuf** : `event_payload` doit désérialiser contre le schéma courant Schema Registry (sinon `REJECTED`).
3. **Validation tenant** : `tenant` doit correspondre au principal SPIFFE du caller (enforcement ACL).
4. **Validation taille** : `event_payload` < 1 MB (sinon `REJECTED`).
5. **Validation idempotency_key** : UUID v4 strict.
6. **Validation occurred_at** : entre `now − 24h` et `now + 5min` (tolérance clock skew).

Un événement rejeté n'entre **jamais** dans l'outbox, ne publie **pas** sur Redpanda, et n'apparaît **pas** dans les streams KAYA. Il est uniquement loggé dans `platform.rejected-events.v1` (rétention 30 j) à des fins de debug.

### 8.9 Logging et tracing ingestion-rs

- Chaque requête produit un `trace_id` W3C propagé en amont (BFF, services Java) et en aval (Redpanda header `traceparent`, KAYA metadata).
- Logs JSON :

```json
{
  "ts": "2026-04-16T10:32:17.104Z",
  "level": "info",
  "service": "ingestion-rs",
  "tenant": "etat-civil",
  "aggregate_id": "dbc5...8f41",
  "idempotency_key": "72fa...b319",
  "event_type": "demande.created.v1",
  "trace_id": "a1b2c3...",
  "span_id": "e4f5a6",
  "kaya_entry_id": "1713260700123-0",
  "redpanda_offset": 4821953,
  "outbox_status": "SENT",
  "duration_ms": 3.2
}
```

---

## 9. Schema Registry + Protobuf — gouvernance des topics durables

### 9.1 Obligation

**Tout topic Redpanda durable est gouverné par Schema Registry.** Format obligatoire : **Protobuf** (pas d'Avro, pas de JSON Schema dans les topics durables).

### 9.2 Mode de compatibilité

`BACKWARD_TRANSITIVE` : une nouvelle version de schéma doit permettre à tout ancien consumer (v1, v2, …) de lire les messages produits par le nouveau schéma (vN). Cela autorise :

- Ajout de champ **optionnel** (avec `[default = ...]` ou `optional`).
- Suppression de champ **en marquant `reserved`**.
- Élargissement de type numérique (int32 → int64).

Interdit :

- Renommage de champ (→ créer vN+1 et faire migration).
- Changement de tag Protobuf.
- Suppression sans `reserved`.

### 9.3 Nommage des topics

```
{projet}.{aggregat}.{event}.v{N}

Exemples :
  etat-civil.dossier.created.v1
  etat-civil.dossier.validated.v2
  hospital.dossier.admitted.v1
  e-ticket.ticket.consumed.v1
  vouchers.voucher.issued.v1
  sogesy.payment.settled.v1
  e-school.enrollment.confirmed.v1
  alt-mission.mission.created.v1
  faso-kalan.lesson.completed.v1
```

### 9.4 Rétention par topic

| Topic famille | Rétention | Justification |
|---------------|-----------|---------------|
| `audit.*` | **10 ans** | Loi 010-2004/AN, exigence audit externe |
| `hospital.*` | **10 ans** | Archivage dossier médical (code de déontologie médicale BF) |
| `sogesy.*` | **10 ans** | BCEAO, réglementation bancaire |
| `vouchers.*` | **7 ans** | Fiscalité + audit campagnes |
| `etat-civil.*` | **10 ans** | Archivage état civil (permanent en Yugabyte) |
| `e-ticket.*` | **90 jours** | Événements éphémères |
| `e-school.*` | **5 ans** | Année scolaire + contentieux |
| `alt-mission.*` | **5 ans** | Rapprochement mission / indemnités |
| `faso-kalan.*` | **2 ans** | Progression pédagogique |

### 9.5 Gouvernance d'évolution

1. Toute évolution de schéma passe par une **Pull Request** sur le dépôt `schema-registry-proto/`.
2. La PR déclenche un check CI : `buf breaking --against main`.
3. Validation manuelle par le CAS pour tout changement `MINOR` ou `MAJOR`.
4. Publication vN+1 en Schema Registry avec `compatibility = BACKWARD_TRANSITIVE`.
5. Communication aux équipes consumer (délai d'adaptation 30 jours avant dépréciation vN−1).

### 9.6 Exemple complet de schéma

```proto
// etat-civil.dossier.created.v2.proto
syntax = "proto3";
package faso.etat_civil.v2;

import "google/protobuf/timestamp.proto";

message DossierCreated {
  string aggregate_id = 1;
  string tenant = 2;
  string idempotency_key = 3;
  google.protobuf.Timestamp occurred_at = 4;

  // v1 avait citizen_name (tag 5) — supprimé, marqué reserved
  reserved 5;
  reserved "citizen_name";

  // v2 : référence opaque au citoyen (UUID seulement, jamais PII en clair)
  string citizen_id = 6;
  ActeType acte_type = 7;
  string commune_code = 8;

  enum ActeType {
    ACTE_TYPE_UNSPECIFIED = 0;
    ACTE_TYPE_NAISSANCE = 1;
    ACTE_TYPE_MARIAGE = 2;
    ACTE_TYPE_DECES = 3;
  }
}
```

### 9.7 Vérification en CI

```yaml
# schema-check workflow (extrait)
- name: Buf lint
  run: buf lint
- name: Buf breaking check
  run: buf breaking --against '.git#branch=main'
- name: Register dry-run
  run: |
    for proto in $(find schemas -name '*.proto'); do
      rpk registry schema check \
        --subject $(basename $proto .proto) \
        --schema-type protobuf \
        --compatibility BACKWARD_TRANSITIVE \
        --file "$proto"
    done
```

### 9.8 Topics opérationnels vs topics de journal légal

| Catégorie | Rétention | Gouvernance | Exemples |
|-----------|-----------|-------------|----------|
| Journal légal | 7-10 ans | Stricte, BACKWARD_TRANSITIVE | `etat-civil.dossier.*`, `sogesy.payment.*`, `hospital.dossier.*` |
| Opérationnel | 30-90 j | Schema Registry obligatoire, FULL permis | `*.pdf.requested.v1`, `*.notify.v1` |
| Éphémère | 1-7 j | Schema Registry optionnel | `platform.heartbeat.v1`, `platform.metrics.v1` |
| Audit | 10 ans | Stricte, immuable | `audit.*` |

---

## 10. Bibliothèque de scripts Rhai — catalogue officiel

Tous les scripts Rhai sont **versionnés dans le dépôt Git `kaya-rhai-scripts/`**, testés unitairement avec `kaya-test-harness`, et chargés au boot de `ingestion-rs` via `SCRIPT LOAD`. L'exécution en production se fait **exclusivement via `EVALSHA <sha>`**. Le verbe `EVAL` (envoi de source brute) est **désactivé par ACL** pour tous les utilisateurs applicatifs.

### 10.1 Catalogue

| Script | SHA (placeholder) | Projets | Rôle |
|--------|--------------------|---------|------|
| `dedup_and_persist.rhai` | `<sha-dedup>` | tous | Dédup idempotency_key + XADD stream |
| `acquire_lock.rhai` | `<sha-acquire>` | tous | Acquisition lock distribué TTL |
| `release_lock.rhai` | `<sha-release>` | tous | Libération lock avec fencing token |
| `worm_lock.rhai` | `<sha-worm>` | ÉTAT-CIVIL, HOSPITAL | Write-Once Read-Many (acte signé immuable) |
| `four_eyes_validate.rhai` | `<sha-4eyes>` | HOSPITAL, SOGESY, VOUCHERS | Validation 2 approbateurs distincts |
| `sliding_window_quota.rhai` | `<sha-quota>` | E-TICKET, ALT-MISSION | Rate-limit glissant par tenant/utilisateur |
| `hmac_chain_advance.rhai` | `<sha-hmac>` | SOGESY | Chaînage HMAC anti-altération transactions |

### 10.2 Exemple — `worm_lock.rhai`

```rhai
// worm_lock.rhai — une fois scellé, toute tentative de réécriture échoue
let key = KEYS[0];
let payload = ARGS[0];
let sealer = ARGS[1];

if kaya.exists(key) {
    let existing = kaya.hgetall(key);
    if existing.sealed == "true" {
        return #{ status: "WORM_REJECTED", sealer: existing.sealer };
    }
}

kaya.hset_multi(key, #{
    payload: payload,
    sealer: sealer,
    sealed: "true",
    sealed_at: kaya.now_ms().to_string()
});
kaya.persist(key);   // jamais d'expiration
return #{ status: "SEALED" };
```

### 10.3 Exemple — `hmac_chain_advance.rhai` (SOGESY)

```rhai
// Chaîne HMAC : chaque transaction SOGESY chaîne son hash au précédent
let chain_key = KEYS[0];          // "sogesy:chain:<merchant_id>"
let tx_id = ARGS[0];
let tx_payload = ARGS[1];
let hmac_key_id = ARGS[2];

let prev = kaya.hget(chain_key, "tail_hmac");
let prev_hmac = if prev == () { "GENESIS" } else { prev };

let new_hmac = kaya.hmac_sha256(hmac_key_id, prev_hmac + "|" + tx_id + "|" + tx_payload);
kaya.hset_multi(chain_key, #{
    tail_hmac: new_hmac,
    tail_tx_id: tx_id,
    tail_at: kaya.now_ms().to_string()
});
kaya.xadd("sogesy:chain:events", "*",
    "tx_id", tx_id, "prev_hmac", prev_hmac, "new_hmac", new_hmac);

return #{ status: "ADVANCED", new_hmac: new_hmac };
```

### 10.4 Tests

Chaque script Rhai dispose d'un fichier `*.rhai.test` exécuté par `kaya-test-harness` (instance KAYA éphémère en CI). Couverture minimale 80 %. Pas de merge sans CI verte.

### 10.5 Exemple — `sliding_window_quota.rhai`

```rhai
// sliding_window_quota.rhai
// KEYS[0] = "quota:<tenant>:<user>:<endpoint>"
// ARGS[0] = window_seconds (ex. 60)
// ARGS[1] = max_requests (ex. 100)

let key = KEYS[0];
let window = ARGS[0].parse_int();
let max_req = ARGS[1].parse_int();
let now_ms = kaya.now_ms();
let cutoff_ms = now_ms - (window * 1000);

// Suppression des éléments hors-fenêtre (ZREMRANGEBYSCORE)
kaya.zremrangebyscore(key, 0, cutoff_ms);

// Comptage
let count = kaya.zcard(key);
if count >= max_req {
    let retry_after = ((kaya.zrange(key, 0, 0, true)[1].parse_int() + (window * 1000)) - now_ms) / 1000;
    return #{ allowed: false, retry_after_s: retry_after, count: count };
}

// Enregistrement de la requête courante
kaya.zadd(key, now_ms, now_ms.to_string());
kaya.expire(key, window + 10);   // GC auto
return #{ allowed: true, count: count + 1, remaining: max_req - count - 1 };
```

### 10.6 Exemple — `release_lock.rhai` avec fencing

```rhai
// release_lock.rhai
// KEYS[0] = "lock:<resource>"
// ARGS[0] = owner_id présumé

let key = KEYS[0];
let claimed_owner = ARGS[0];

let current = kaya.get(key);
if current == () {
    return #{ released: false, reason: "NOT_HELD" };
}
if current != claimed_owner {
    return #{ released: false, reason: "NOT_OWNER", actual_owner: current };
}
kaya.del(key);
return #{ released: true };
```

### 10.7 Chargement et déploiement

```shell
# Au boot de ingestion-rs
for script in /opt/kaya-rhai-scripts/*.rhai; do
  SHA=$(kaya-cli SCRIPT LOAD "$(cat $script)")
  echo "$(basename $script .rhai)=$SHA" >> /var/run/ingestion-rs/scripts.env
done

# Usage au runtime (Rust, ingestion-rs)
# let sha = env::var("SCRIPT_DEDUP_AND_PERSIST")?;
# kaya_client.evalsha(&sha, &keys, &args).await?;
```

### 10.8 Politique de versionnement

Chaque script est **versionné** par son SHA (`EVALSHA` sur une source donnée renvoie un SHA déterministe) :

- `scripts.env` est régénéré à chaque déploiement.
- Un audit mensuel croise le registre Git et les scripts effectivement chargés en prod via `SCRIPT EXISTS`.
- Aucun script "orphelin" (non présent dans Git mais présent dans KAYA) ne doit exister. Détection via `kaya-script-auditor`.

---

## 11. Matrice d'application par sous-projet

### 11.1 Vue d'ensemble

| Sous-projet | Couche critique | RPO contractuel | Patterns Rhai | Spécificités |
|-------------|-----------------|-----------------|---------------|--------------|
| **ÉTAT-CIVIL** | Durable (Yugabyte) + Légal | 0 | `dedup_and_persist`, `worm_lock` (acte signé) | Archivage 10 ans, signature numérique, WORM |
| **HOSPITAL** | Durable + Légal | 0 | `dedup_and_persist`, `four_eyes_validate`, `worm_lock` | PII ultra-sensibles, chiffrement colonne, four-eyes médecin+pharmacien |
| **E-TICKET** | Hot path (KAYA) | ≤ 1 s | `dedup_and_persist`, `sliding_window_quota`, `acquire_lock` | Anti-doublon Bloom, rate-limit burst billetterie |
| **VOUCHERS** | Durable + Légal | 0 | `dedup_and_persist`, `four_eyes_validate` (émission), `worm_lock` (consommation) | Anti-fraude, Bloom 200k, HMAC par code |
| **SOGESY** | Légal + Durable | **0** (fsync always) | `dedup_and_persist`, `four_eyes_validate`, `hmac_chain_advance` | BCEAO, chaîne HMAC, rétention 10 ans |
| **E-SCHOOL** | Durable | ≤ 1 s | `dedup_and_persist`, `acquire_lock` (session) | Pic rentrée scolaire, scale horizontale KAYA |
| **ALT-MISSION** | Légal | 0 | `dedup_and_persist`, `four_eyes_validate`, `sliding_window_quota` | Validation hiérarchique 2 niveaux |
| **FASO-KALAN** | Hot path | ≤ 1 s | `dedup_and_persist`, `acquire_lock` | Sessions pédagogiques |

### 11.2 Microservices par sous-projet (gabarit canonique ÉTAT-CIVIL)

| Microservice | Langue | Rôle | Stores |
|--------------|--------|------|--------|
| `ingestion-rs` | Rust | Seul scripteur KAYA + Yugabyte | KAYA (W), Yugabyte (W) |
| `demande-ms` | Java 21 / DGS | API GraphQL demande de dossier | gRPC → ingestion-rs |
| `traitement-acte-ms` | Java 21 | Workflow de traitement métier | gRPC → ingestion-rs, Yugabyte (R) |
| `validation-acte-ms` | Java 21 | Four-eyes validation | KAYA (R via EVALSHA four_eyes), gRPC → ingestion-rs |
| `impression-ms` | Java 21 | Demande PDF | Kafka produce `*.pdf.requested.v1` |
| `notify-ms` | Java 21 | Notifications (SMS/email) | Kafka consume `*.pdf.ready.v1`, `*.notify.v1` |
| `audit-ms` | Java 21 | Miroir topic audit → Yugabyte read-only | Redpanda consume, Yugabyte (W audit table) |
| `cert-render-ms` | Rust (Typst) | Rendu PDF typographié | Input Kafka, output S3 souverain |
| `outbox-relay` | Rust | Transactional outbox dispatcher | Yugabyte (R/W outbox), KAYA (W), Redpanda (W) |

Ce gabarit est **réutilisé** par HOSPITAL (avec `dossier-medical-ms`, `prescription-ms`, `pharmacie-ms`), E-TICKET (`billetterie-ms`, `checkin-ms`), VOUCHERS (`emission-ms`, `consommation-ms`), SOGESY (`paiement-ms`, `rapprochement-ms`), E-SCHOOL (`inscription-ms`, `emploi-du-temps-ms`), ALT-MISSION (`mission-ms`, `indemnite-ms`), FASO-KALAN (`lecon-ms`, `progression-ms`).

### 11.3 ÉTAT-CIVIL — flux détaillé

1. Citoyen dépose demande via mairie numérique (Angular).
2. `demande-ms` persiste la demande (projetée via ingestion-rs : `XADD etat-civil:demandes`, insert outbox, produce `etat-civil.demande.created.v1`).
3. `traitement-acte-ms` consume le topic, orchestre via Temporal un workflow `DossierWorkflow`.
4. Officier d'état civil valide via `validation-acte-ms` (four-eyes avec supervisant).
5. Si `APPROVED`, `worm_lock.rhai` scelle l'acte dans KAYA, produce `etat-civil.acte.sealed.v1`.
6. `cert-render-ms` génère le PDF Typst, upload MinIO, signature XAdES.
7. `notify-ms` envoie SMS/email au citoyen.

### 11.4 HOSPITAL — spécificités

- **PII ultra-sensibles** : nom, diagnostic, prescriptions sont chiffrés colonne-à-colonne (TDE AES-256-GCM), clé par établissement hospitalier.
- **Four-eyes** obligatoire pour : administration traitement stupéfiants, sortie patient mineur, transfert inter-hôpitaux.
- **Bloom filter** : `bf:hospital:dossiers` pour vérif rapide existence patient (500k / 0.01%).
- **Temporal** : workflows longs pour suivis post-opératoires, rappels médicamenteux, rendez-vous.
- **Topic rétention** 10 ans (code déontologie).

### 11.5 SOGESY — spécificités BCEAO

- **`fsync always`** sur KAYA.
- **Chaîne HMAC** (`hmac_chain_advance.rhai`) : chaque transaction calcule `HMAC(prev_hash | tx_id | payload)`. Invalidation détectée immédiatement si la chaîne est rompue.
- **Four-eyes** pour toute transaction > 500 000 XOF (plafond configurable).
- **Rapprochement bancaire quotidien** : workflow Temporal `SogesyReconciliationWorkflow` à 23h00 consomme les topics, compare avec Yugabyte, produit rapport signé.
- **Rétention** 10 ans.

### 11.6 E-TICKET — spécificités

- Anti-doublon via Bloom `bf:eticket:consumed` (1M / 0.01%).
- `sliding_window_quota` pour prévenir scalping (max 10 tickets / 60 s / utilisateur).
- `acquire_lock` pour réserver un siège spécifique (TTL 120 s).
- Rétention 90 j (éphémère).

### 11.7 VOUCHERS — spécificités

- Génération codes : algorithme cryptographique Vault Transit (pas de séquentiel prédictible).
- `four_eyes_validate` sur émission massive (campagnes > 1000 vouchers).
- `worm_lock` sur consommation : un voucher consommé ne peut être re-consommé.
- Rétention 7 ans (fiscalité).

### 11.8 E-SCHOOL, ALT-MISSION, FASO-KALAN

Ces trois projets suivent le gabarit canonique sans spécificités majeures. Principales adaptations :

- **E-SCHOOL** : burst de rentrée (5000 evt/min pendant 30 min) → scaling auto de `ingestion-rs` (HPA Kubernetes CPU > 70 %).
- **ALT-MISSION** : four-eyes hiérarchique (chef de service + directeur) pour validation missions.
- **FASO-KALAN** : session pédagogique = lock `acquire_lock` sur la leçon + l'élève pendant la durée.

### 11.9 Tableau croisé : patterns KAYA par sous-projet

| Pattern | ÉTAT-CIVIL | HOSPITAL | E-TICKET | VOUCHERS | SOGESY | E-SCHOOL | ALT-MISSION | FASO-KALAN |
|---------|:----------:|:--------:|:--------:|:--------:|:------:|:--------:|:-----------:|:----------:|
| `dedup_and_persist` | X | X | X | X | X | X | X | X |
| `acquire_lock`/`release_lock` | X | X | X | — | X | X | X | X |
| `worm_lock` | X | X | — | X | X | — | — | — |
| `four_eyes_validate` | X | X | — | X | X | — | X | — |
| `sliding_window_quota` | — | — | X | X | X | — | X | X |
| `hmac_chain_advance` | — | — | — | — | X | — | — | — |
| Bloom filter | X | X | X | X | X | X | — | — |
| Stream KAYA | X | X | X | X | X | X | X | X |
| Vue matérialisée | X | X | X | X | X | X | — | — |

### 11.10 Topics Redpanda consolidés

```text
# ÉTAT-CIVIL
etat-civil.demande.created.v1        # 10 ans
etat-civil.dossier.validated.v1      # 10 ans
etat-civil.acte.sealed.v1            # 10 ans
etat-civil.pdf.requested.v1          # 30 j
etat-civil.pdf.ready.v1              # 90 j
etat-civil.notify.v1                 # 30 j

# HOSPITAL
hospital.dossier.admitted.v1         # 10 ans
hospital.prescription.issued.v1      # 10 ans
hospital.dossier.discharged.v1       # 10 ans
hospital.four-eyes.v1                # 10 ans

# E-TICKET
e-ticket.ticket.issued.v1            # 90 j
e-ticket.ticket.consumed.v1          # 90 j

# VOUCHERS
vouchers.voucher.issued.v1           # 7 ans
vouchers.voucher.consumed.v1         # 7 ans

# SOGESY
sogesy.payment.initiated.v1          # 10 ans
sogesy.payment.settled.v1            # 10 ans
sogesy.chain.hmac.v1                 # 10 ans

# E-SCHOOL
e-school.enrollment.confirmed.v1     # 5 ans
e-school.grade.recorded.v1           # 5 ans

# ALT-MISSION
alt-mission.mission.created.v1       # 5 ans
alt-mission.indemnite.paid.v1        # 5 ans

# FASO-KALAN
faso-kalan.lesson.started.v1         # 2 ans
faso-kalan.lesson.completed.v1       # 2 ans

# TRANSVERSES
audit.pii-access.v1                  # 10 ans
audit.reconciliation.v1              # 10 ans
platform.logs.v1                     # 30 j
platform.heartbeat.v1                # 7 j
```

---

## 12. Matrice de décision Rhai / MULTI-EXEC / WATCH-CAS / Pipeline par cas d'usage

| Cas d'usage | Rhai | MULTI/EXEC | WATCH/CAS | Pipeline | Recommandation |
|-------------|:----:|:----------:|:---------:|:--------:|----------------|
| Dédup idempotency_key + XADD stream | OUI | partiel | non | non | **Rhai `dedup_and_persist`** |
| Acquisition lock TTL + owner | OUI | non | partiel | non | **Rhai `acquire_lock`** |
| Release lock avec fencing | OUI | non | OUI | non | **Rhai `release_lock`** (fencing token) |
| Incrément compteur simple | OUI | OUI | OUI | non | **INCR** direct (pas besoin de script) |
| Bulk préchauffage cache (1000 SET) | non | non | non | OUI | **Pipeline** |
| Four-eyes : 2 approuvés distincts + transition | OUI | non | non | non | **Rhai `four_eyes_validate`** |
| Quota glissant (rate-limit) | OUI | non | non | non | **Rhai `sliding_window_quota`** |
| Chaînage HMAC SOGESY | OUI | non | non | non | **Rhai `hmac_chain_advance`** |
| Sceller WORM (acte état civil) | OUI | non | non | non | **Rhai `worm_lock`** |
| Lecture multi-clés indépendantes | non | partiel | non | OUI | **Pipeline MGET / batch GET** |
| Transaction simple SET + EXPIRE | non | OUI | non | non | **SET ... EX** atomique natif |
| Vérification BF + GET | non | non | non | OUI | **Pipeline BF.EXISTS + GET** |

---

## 13. Pipeline PDF asynchrone

### 13.1 Architecture

```
impression-ms (Java)
  └── produce Kafka: etat-civil.pdf.requested.v1
           │
           ▼
      pdf-worker-ms (Rust + Typst, GraalVM non pertinent ici)
           │  1. consume .pdf.requested.v1
           │  2. SET NX pdf:{dossier_id} EX 86400  (lock idempotence)
           │  3. render Typst → PDF
           │  4. PUT S3 souverain (MinIO) + hash SHA-256
           │  5. produce: etat-civil.pdf.ready.v1 (url, sha256, dossier_id)
           ▼
      notify-ms (Java)
           └── consume .pdf.ready.v1 → envoi SMS/email avec URL signée
```

### 13.2 Contraintes

- **Span request (producer side)** : < 5 ms (lock KAYA + produce Redpanda).
- **Rendu Typst** : < 500 ms P95 (alerte si > 500 ms).
- **Retention S3** : cf. §9 (10 ans pour HOSPITAL, ÉTAT-CIVIL).
- **URL signée** : TTL 24 h, clé HMAC dans Vault.

### 13.3 Lock idempotence KAYA

```shell
kaya-cli SET pdf:{dossier_id} <worker_id> NX EX 86400
# si false → un autre worker a déjà le rendu en cours, skip
```

### 13.4 Topics

| Topic | Rétention | Clé partition |
|-------|-----------|---------------|
| `{projet}.pdf.requested.v1` | 30 j | `dossier_id` |
| `{projet}.pdf.ready.v1` | 90 j | `dossier_id` |
| `{projet}.pdf.failed.v1` | 30 j | `dossier_id` |

### 13.5 Rendu Typst — exemple

```typst
// acte-naissance.typ
#set document(title: "Acte de Naissance")
#set page(paper: "a4", margin: 2cm)
#set text(font: "Liberation Serif", size: 11pt)

#align(center)[
  #text(size: 16pt, weight: "bold")[RÉPUBLIQUE DU BURKINA FASO]\
  #text(size: 12pt)[Unité - Progrès - Justice]\
  \
  #text(size: 18pt, weight: "bold")[ACTE DE NAISSANCE]
]

#let data = json.decode(sys.inputs.data)

*Numéro* : #data.numero\
*Nom* : #data.nom\
*Prénoms* : #data.prenoms\
*Date de naissance* : #data.date_naissance\
*Lieu* : #data.lieu\
*Commune* : #data.commune\

#align(right)[
  Fait à #data.commune, le #data.date_emission\
  L'Officier d'État Civil
]

#align(right)[
  QR : #qr(data.verification_url, size: 3cm)
]
```

Compilation : `typst compile --input data=@dossier.json acte-naissance.typ acte.pdf`. Temps typique : 40-80 ms.

### 13.6 Signature XAdES

Après rendu PDF, `cert-render-ms` signe le PDF via **XAdES-B-LT** (long-term) avec certificat délivré par l'AC souveraine du Burkina Faso (ARCEP-BF). Le certificat de signature est rafraîchi tous les 2 ans.

---

## 14. Transactional Outbox opérationnel

### 14.1 Table `outbox` YugabyteDB

Partitionnée **par jour** (`PARTITION BY RANGE (created_date)`) pour borner la taille.

```sql
CREATE TABLE outbox (
  id              UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
  aggregate_id    UUID         NOT NULL,
  partition_key   TEXT         NOT NULL,
  idempotency_key UUID         NOT NULL,
  topic           TEXT         NOT NULL,
  payload         BYTEA        NOT NULL,       -- Protobuf encoded
  status          TEXT         NOT NULL,       -- PENDING | SENT | DEAD_LETTER
  retry_count     INT          NOT NULL DEFAULT 0,
  error_reason    TEXT,
  created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
  created_date    DATE         NOT NULL DEFAULT current_date,
  sent_at         TIMESTAMPTZ,
  UNIQUE (idempotency_key)
) PARTITION BY RANGE (created_date);

CREATE INDEX outbox_pending_idx ON outbox (created_at)
  WHERE status = 'PENDING';

CREATE INDEX outbox_dlq_idx ON outbox (created_at)
  WHERE status = 'DEAD_LETTER';
```

### 14.2 Service `outbox-relay`

- Implémentation : **Rust**, binaire natif.
- Déploiement : **3 workers × 2 instances** (6 pods) en HA actif/actif.
- Partitionnement logique : `hash(aggregate_id) mod N_WORKERS` ⇒ chaque worker prend un shard de `aggregate_id`.
- Polling SQL :

```sql
SELECT id, topic, payload, retry_count
FROM outbox
WHERE status = 'PENDING'
  AND (hash_text(aggregate_id::text) % :n_workers) = :worker_idx
ORDER BY created_at ASC
FOR UPDATE SKIP LOCKED
LIMIT 100;
```

### 14.3 Ordre de publication (immuable)

```text
1. KAYA XADD <projection-stream> MAXLEN ~ N * event
2. Redpanda PRODUCE <topic> acks=all idempotent=true
3. UPDATE outbox SET status='SENT', sent_at=NOW() WHERE id=:id
```

### 14.4 Backoff et DLQ

```text
Retry 1 : +200 ms
Retry 2 : +400 ms
Retry 3 : +800 ms
Retry 4 : +1600 ms
Retry 5 : +3200 ms
Retry 6 : → DEAD_LETTER (alerte P1)
```

### 14.5 Runbook DLQ (6 étapes)

1. **Détection** : alerte `outbox_relay_dead_letter_count > 0` en Prometheus.
2. **Triage** : requête `SELECT topic, error_reason, COUNT(*) FROM outbox WHERE status='DEAD_LETTER' GROUP BY topic, error_reason;`
3. **Root cause** : corréler `error_reason` avec logs Redpanda / KAYA / Schema Registry.
4. **Correction** : soit patch applicatif, soit mise à jour de schéma, soit purge.
5. **Replay** : `UPDATE outbox SET status='PENDING', retry_count=0 WHERE id IN (...)` après validation CAS.
6. **Post-mortem** : dans les 72 h, ADR publié, leçons intégrées aux tests chaos.

### 14.6 Pseudo-code du worker outbox-relay (Rust)

```rust
// outbox-relay/src/worker.rs (extrait conceptuel)
async fn run_worker(worker_idx: u32, n_workers: u32) -> Result<()> {
    loop {
        let batch = yb.query(
            "SELECT id, aggregate_id, topic, payload, retry_count
             FROM outbox
             WHERE status = 'PENDING'
               AND (hash_text(aggregate_id::text) % $1) = $2
             ORDER BY created_at ASC
             FOR UPDATE SKIP LOCKED
             LIMIT 100",
            &[&(n_workers as i64), &(worker_idx as i64)],
        ).await?;

        for row in batch {
            let id: Uuid = row.get("id");
            let topic: String = row.get("topic");
            let payload: Vec<u8> = row.get("payload");
            let retry: i32 = row.get("retry_count");

            // (1) KAYA XADD
            let kaya_result = kaya.xadd(&format!("{}:stream", topic), &payload).await;
            if kaya_result.is_err() {
                increment_retry_or_dlq(id, retry, "kaya_xadd_failed").await?;
                continue;
            }

            // (2) Redpanda PRODUCE acks=all
            let rp_result = producer.send(FutureRecord::to(&topic)
                .payload(&payload)
                .key(&id.to_string().as_bytes()[..])).await;
            if rp_result.is_err() {
                increment_retry_or_dlq(id, retry, "redpanda_produce_failed").await?;
                continue;
            }

            // (3) UPDATE outbox SET status='SENT'
            yb.execute(
                "UPDATE outbox SET status='SENT', sent_at=now() WHERE id = $1",
                &[&id]
            ).await?;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn increment_retry_or_dlq(id: Uuid, retry: i32, reason: &str) -> Result<()> {
    let backoffs = [200, 400, 800, 1600, 3200];
    if retry >= backoffs.len() as i32 {
        yb.execute(
            "UPDATE outbox SET status='DEAD_LETTER', error_reason=$1 WHERE id=$2",
            &[&reason, &id]
        ).await?;
        metric_inc("outbox_relay_dead_letter_count");
    } else {
        let delay_ms = backoffs[retry as usize];
        yb.execute(
            "UPDATE outbox SET retry_count=retry_count+1,
                               error_reason=$1
             WHERE id=$2",
            &[&reason, &id]
        ).await?;
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }
    Ok(())
}
```

### 14.7 Garanties globales

- **At-least-once delivery** côté Redpanda (grâce à `acks=all` + idempotent producer).
- **Exactly-once effective** grâce à l'idempotency_key + déduplication côté consumer via la table `processed_events` (clé : `(topic, offset)` ou `idempotency_key`).
- **Ordre strict par `aggregate_id`** (car même worker relay traite un `aggregate_id` donné, et même partition Redpanda ciblée).

---

## 15. DR / BCP — backups, restore, chaos

### 15.1 Matrice de backups

| Composant | Fréquence | Rétention | Cible | Chiffrement |
|-----------|-----------|-----------|-------|-------------|
| **KAYA snapshots** | toutes les 6 h | 30 jours | MinIO souverain | AES-256-GCM (Vault Transit) |
| **YugabyteDB full** | quotidien 02h00 | 90 jours | MinIO souverain + site DR | AES-256-GCM |
| **YugabyteDB incrémental** | toutes les 15 min (WAL) | 7 jours | MinIO | AES-256-GCM |
| **Redpanda Tiered Storage** | continu (offloading segments) | selon rétention topic | S3 souverain | TLS + AES-256 |
| **Vault HSM** | quotidien snapshot Raft + transit | 1 an + HSM physique | Coffre physique BF | HSM (FIPS 140-2 L3) |
| **Scripts Rhai** | à chaque merge | illimitée | Git (4 miroirs BF) | signature GPG |

### 15.2 Ordre de restore

```text
1. Redpanda     (journal légal)       → RTO 10 min
2. YugabyteDB   (vérité durable)      → RTO 15 min
3. reconciliator-rs
     └── compare Redpanda ↔ Yugabyte sur fenêtre incident
     └── ré-applique événements manquants
4. KAYA warm-up (24 h)
     └── projections reconstruites depuis Redpanda
     └── Bloom filters reconstruits depuis Yugabyte
5. Validation smoke tests (read-only)
6. Trafic progressif : 10 % → 50 % → 100 %
```

### 15.3 Chaos engineering trimestriel

7 scénarios obligatoires, exécutés en pré-production par trimestre :

| # | Scénario | Injection | Critère succès |
|---|----------|-----------|----------------|
| 1 | Perte nœud KAYA primaire | kill -9 primaire | Failover réplica < 5 min, perte ≤ 1 s |
| 2 | Perte broker Redpanda | iptables drop | Quorum maintenu, 0 perte |
| 3 | Perte TServer YugabyteDB | kill TServer | RAFT re-leader, 0 perte |
| 4 | Partition réseau KAYA ↔ services | tc netem partition 5 min | Circuit breakers ouvrent, mode dégradé |
| 5 | Corruption AOF KAYA | tronquer AOF | Détection au boot, restore snapshot |
| 6 | Crash outbox-relay (tous workers) | drain + block | Reprise auto sous 2 min, pas de duplicata |
| 7 | Rotation clé Vault en cours de trafic | vault rotate transit | 0 erreur applicative |

Résultats consignés dans le dépôt `chaos-reports/YYYY-Qn/`.

### 15.4 Procédure de restore détaillée — Redpanda

```shell
# 1. Arrêt cluster actuel (si corrompu)
systemctl stop redpanda

# 2. Nettoyage data dir
rm -rf /var/lib/redpanda/data/*

# 3. Restauration Tiered Storage (métadonnées + segments S3)
rpk cluster storage restore \
  --s3-bucket faso-souverain-redpanda \
  --s3-region bf-ouaga \
  --restore-timestamp "2026-04-15T00:00:00Z"

# 4. Démarrage cluster
systemctl start redpanda

# 5. Vérification
rpk cluster health
rpk topic list

# 6. Validation métier : consume dernier offset topics critiques
rpk topic consume etat-civil.dossier.created.v1 -n 10
```

### 15.5 Procédure de restore — YugabyteDB

```shell
# Point-in-time restore à T−15min avant incident
yb-admin -master_addresses $MASTERS create_snapshot_schedule 30 365 "ysql.prod"
yb-admin -master_addresses $MASTERS restore_snapshot_schedule <schedule_id> "2026-04-15T08:45:00Z"

# Vérification intégrité
ysqlsh -c "SELECT COUNT(*) FROM outbox WHERE status='PENDING';"
ysqlsh -c "SELECT COUNT(*) FROM actes_etat_civil;"
```

### 15.6 Warm-up KAYA (24 h)

Après restauration Yugabyte et Redpanda, KAYA est vide. Procédure :

1. Start KAYA nodes (vide).
2. `ingestion-rs` bascule en mode `warm-up` : les reads renvoient directement depuis Yugabyte.
3. Un job `kaya-warmup` consomme les topics Redpanda depuis `earliest` filtré à `now - 24h`, ré-applique les projections.
4. Bloom filters reconstruits via `BF.MADD` bulk depuis les clés présentes en Yugabyte.
5. Mode `warm-up` désactivé une fois le lag consumer < 1 s.

### 15.7 Plan de continuité d'activité (BCP)

| Scénario | Composant touché | Stratégie |
|----------|------------------|-----------|
| Panne site primaire (DC-Ouaga) | Tout | Bascule DC-Bobo, RTO global ≤ 30 min |
| Perte cluster KAYA | KAYA | Reprise via snapshots S3 (6h) + warm-up Redpanda |
| Perte cluster Yugabyte | Yugabyte | Restore full + incrémental, RTO 15 min |
| Perte cluster Redpanda | Redpanda | Tiered Storage replay, RTO 10 min |
| Perte Vault | Vault | Unseal 3 opérateurs, Raft Integrated Storage |
| Cyberattaque ransomware | Tous | Isolation, restore depuis snapshots offline chiffrés |
| Coupure liaison DC | Connectivité inter-site | Mode split-brain géré par KAYA-Sentinel (quorum), Redpanda RAFT perd quorum si 2/3 brokers isolés |

---

## 16. Observabilité — Prometheus, Grafana, Jaeger

### 16.1 Métriques clés et seuils d'alerte

| Métrique | Seuil alerte | Criticité | Dashboard |
|----------|--------------|-----------|-----------|
| `redpanda_produce_latency_ms` (P99) | **> 15 ms** | P2 | Redpanda |
| `kaya_xadd_latency_ms` (P99) | **> 5 ms** | P2 | KAYA |
| `kaya_rhai_exec_latency_ms` (P99) | **> 10 ms** | P2 | KAYA |
| `kaya_persistence_enabled` | **== 0** | **P1 CRITIQUE** | KAYA |
| `outbox_relay_pending_count` | **> 100** | P2 | Outbox |
| `outbox_relay_dead_letter_count` | **> 0** | **P1** | Outbox |
| `outbox_relay_lag_seconds` (P99) | **> 30 s** | P2 | Outbox |
| `pdf_generation_duration_ms` (P95) | **> 500 ms** | P3 | PDF |
| `grpc_server_handling_seconds` (P99) | **> 100 ms** | P2 | Services |
| `yugabytedb_query_duration_ms` (P99) | **> 50 ms** | P2 | Yugabyte |
| `notification_delivery_success_rate` | **< 99 %** | P2 | Notify |
| `spire_svid_ttl_seconds` | **< 900** (15 min) | P2 | Identity |
| `consumer_lag_seconds` (P99) | **> 60 s** | P2 | Redpanda |

### 16.2 Dashboards Grafana (7 obligatoires)

1. **Vue d'ensemble écosystème** — santé globale, erreurs/min, débit, SLO burn rate.
2. **KAYA** — latences XADD/EVALSHA, mem, AOF, réplication lag, snapshots.
3. **Redpanda** — produce/consume latency, consumer lag, partition health, tiered offload.
4. **YugabyteDB** — latence requêtes, RAFT lag, tablets, connexions.
5. **Outbox & Relay** — PENDING/SENT/DLQ counts, lag, throughput.
6. **ARMAGEDDON & Identity** — taux JWT OK/KO, latence WAF Coraza, OPA decisions, SPIRE SVID TTL.
7. **PDF & Notifications** — rendu Typst, taille PDF, livraison SMS/email.

### 16.3 Tracing

- **Jaeger** collecte via **OpenTelemetry**.
- Tous les services émettent un span racine par requête GraphQL, propagé via W3C Trace Context jusqu'à ingestion-rs, KAYA (via tag metadata) et Redpanda (header `traceparent`).
- Rétention traces : 14 jours chaud, 90 jours froid (S3).

### 16.4 Logs

- Format : **JSON structuré** (timestamp, level, service, tenant, trace_id, span_id, message).
- Ingestion : agents vector.dev → Redpanda topic `platform.logs.v1` → Loki (option) ou pipeline Yugabyte `log_archive`.
- **Interdiction** : aucune PII en clair dans les logs (filtre vector obligatoire).

### 16.5 Règles d'alerte Prometheus (exemples complets)

```yaml
groups:
- name: kaya-criticals
  interval: 30s
  rules:
  - alert: KayaPersistenceDisabled
    expr: kaya_persistence_enabled == 0
    for: 1m
    labels:
      severity: P1
      team: sre-infra
    annotations:
      summary: "KAYA persistence disabled on {{ $labels.instance }}"
      runbook: "https://runbooks.faso.bf/kaya/persistence-disabled"

  - alert: KayaRhaiLatencyHigh
    expr: histogram_quantile(0.99, rate(kaya_rhai_exec_latency_ms_bucket[5m])) > 10
    for: 5m
    labels:
      severity: P2
      team: sre-infra
    annotations:
      summary: "KAYA Rhai exec P99 > 10ms"

  - alert: KayaReplicationLagHigh
    expr: kaya_replication_lag_seconds > 1
    for: 2m
    labels:
      severity: P2
    annotations:
      summary: "KAYA replication lag > 1s on {{ $labels.instance }}"

- name: outbox-criticals
  interval: 30s
  rules:
  - alert: OutboxDeadLetter
    expr: outbox_relay_dead_letter_count > 0
    for: 1m
    labels:
      severity: P1
    annotations:
      summary: "{{ $value }} messages in outbox DLQ"
      runbook: "https://runbooks.faso.bf/outbox/dlq-treatment"

  - alert: OutboxPendingBacklog
    expr: outbox_relay_pending_count > 100
    for: 5m
    labels:
      severity: P2
    annotations:
      summary: "Outbox pending backlog > 100"

- name: redpanda-criticals
  interval: 30s
  rules:
  - alert: RedpandaUnderReplicated
    expr: redpanda_under_replicated_partitions > 0
    for: 2m
    labels:
      severity: P1
    annotations:
      summary: "Redpanda partitions under-replicated"
```

### 16.6 SLO et error budgets

| Service | SLO (disponibilité) | Error budget mensuel | Mesure |
|---------|---------------------|----------------------|--------|
| ARMAGEDDON | 99.95 % | 22 min | Ratio 2xx+3xx / total |
| ingestion-rs | 99.95 % | 22 min | Ratio `status=OK` / total |
| KAYA | 99.99 % | 4 min | Ratio uptime |
| Redpanda | 99.95 % | 22 min | Ratio `produce_success` |
| Yugabyte | 99.95 % | 22 min | Ratio `query_success` |
| PDF pipeline | 99.5 % | 3h36 | Ratio `pdf.ready` / `pdf.requested` |

Budget consommé > 75 % → **gel des déploiements non-critiques** jusqu'à récupération.

### 16.6bis Calcul du burn rate

Le **burn rate** quantifie la vitesse à laquelle l'error budget est consommé :

```
burn_rate(window) = error_rate(window) / (1 - SLO)

Exemple SLO 99.95 % (soit 0.05 % erreurs tolérées) :
  - burn_rate = 1  → consomme budget linéairement (épuisé en 30 j)
  - burn_rate = 14 → consomme budget en 2 j (alerte rapide)
  - burn_rate = 2  → consomme budget en 15 j (alerte slow)
```

Règles d'alerte multi-window (Google SRE Workbook) :

| Fenêtre | Burn rate | Criticité | Action |
|---------|-----------|-----------|--------|
| 1 h | > 14 | P1 | Page immédiate |
| 6 h | > 6 | P1 | Page immédiate |
| 24 h | > 3 | P2 | Ticket |
| 3 j | > 1 | P3 | Revue hebdomadaire |

### 16.7 Dashboards Grafana — détail des panneaux

#### Dashboard 1 : Vue d'ensemble

- Carte santé composants (vert/jaune/rouge) : KAYA, Redpanda, Yugabyte, Temporal, Vault, SPIRE, ARMAGEDDON.
- Graphe `requests_per_second` par tenant, empilé.
- Graphe `errors_per_minute` par service, couleur par criticité.
- Gauge SLO burn rate (1h, 6h, 24h) par service critique.
- Liste alertes actives (Prometheus Alertmanager).

#### Dashboard 2 : KAYA

- Latence XADD P50/P99/P999 par shard.
- Latence EVALSHA P50/P99 par script.
- Mémoire utilisée / limite par nœud.
- Nombre connexions actives.
- AOF backlog (octets non flushés).
- Snapshots : dernier timestamp, durée, taille.
- Réplication lag primaire → réplica.
- Taux hit/miss sur Bloom filters (`bf_hit_total` / `bf_miss_total`).

#### Dashboard 3 : Redpanda

- Produce rate par topic.
- Consume rate par consumer group.
- Consumer lag par consumer group (seuil alerte 60 s).
- Under-replicated partitions.
- Tiered storage offload rate.
- Latence produce P99 par topic.
- Disque utilisé par broker.

#### Dashboard 4 : YugabyteDB

- Requêtes/s par type (SELECT, INSERT, UPDATE, DELETE).
- Latence P99 requête par type.
- Tablets répartition.
- RAFT leader-election rate.
- Connexions actives.
- Cache hit ratio.

#### Dashboard 5 : Outbox

- Counts PENDING / SENT / DEAD_LETTER en temps réel.
- Age du plus vieux message PENDING.
- Throughput publication (messages/min).
- Lag P99 entre `created_at` et `sent_at`.
- Breakdown DLQ par `error_reason`.

#### Dashboard 6 : ARMAGEDDON & Identity

- Requêtes/s par endpoint.
- Taux de rejet JWT invalide.
- Latence WAF Coraza P99.
- Décisions OPA (allow/deny) par politique.
- SVID TTL distribution par workload.
- Certificats Vault expirant dans < 7 j.

#### Dashboard 7 : PDF & Notifications

- PDF requested / ready / failed par minute.
- Temps de rendu Typst P50/P95.
- Taille PDF P95.
- Livraison notifications : success rate par canal (SMS/email/push).
- Latence bout-en-bout : depuis `etat-civil.pdf.requested.v1` jusqu'à `notify.delivered.v1`.

### 16.8 Alerting escalation

| Sévérité | Premier répondant | Escalation (15 min si non ack) | Escalation finale (45 min) |
|----------|-------------------|-------------------------------|----------------------------|
| P1 CRITIQUE | SRE on-call | Lead SRE | CTO + CAS |
| P2 | SRE on-call | Lead SRE | — |
| P3 | Équipe produit | SRE on-call (heures bureau) | — |

Canaux :

- PagerDuty → SMS + appel vocal (P1/P2).
- Mattermost channel `#incidents` (tous).
- Email digest quotidien (P3).

---

## 17. Modes dégradés — runbooks par composant

### 17.1 KAYA primaire down

| Aspect | Comportement |
|--------|--------------|
| Impact | Écritures hot path bloquées |
| Détection | `kaya_up == 0` sur primaire |
| Mode dégradé | Failover auto vers réplica (port 6381 promu primaire) |
| Écritures pendant bascule (≤ 60 s) | Buffer mémoire ingestion-rs (circuit-breaker ouvert, `RESOURCE_EXHAUSTED` renvoyé au client) |
| Reprise | Ré-attachement ancien primaire en réplica après repair, vérification AOF |
| RPO effectif | ≤ 1 s (fsync everysec) |

### 17.2 Redpanda quorum perdu (< 2 brokers)

| Aspect | Comportement |
|--------|--------------|
| Impact | Produce bloqué (`acks=all` exige min.insync.replicas=2) |
| Détection | `redpanda_under_replicated_partitions > 0` |
| Mode dégradé | ingestion-rs bascule **writes en outbox PENDING** uniquement (pas de publication) + alerte P1 |
| Reprise | Restauration quorum → outbox-relay rejoue dans l'ordre |
| RPO effectif | 0 (outbox garantit) |

### 17.3 YugabyteDB primaire down

| Aspect | Comportement |
|--------|--------------|
| Impact | Lectures dégradées, écritures peuvent ralentir |
| Détection | `yb_tserver_up == 0` ou `raft_leader_election > 5s` |
| Mode dégradé | Lectures servies par réplica synchrone RAFT, écritures re-leader-élection |
| Reprise | Auto (RAFT), < 10 s |
| RPO | 0 |

### 17.4 Outbox Relay tous workers down

| Aspect | Comportement |
|--------|--------------|
| Impact | Backlog outbox PENDING grossit |
| Détection | `outbox_relay_pending_count > 100` croissant |
| Mode dégradé | Kubernetes relance automatique pods ; alerte P1 si toujours down > 2 min |
| Reprise | Polling reprend, `FOR UPDATE SKIP LOCKED` évite duplications |
| RPO | 0 (données dans Yugabyte) |

### 17.5 Temporal down

| Aspect | Comportement |
|--------|--------------|
| Impact | Workflows longs suspendus (four-eyes, rapprochements) |
| Détection | `temporal_frontend_up == 0` |
| Mode dégradé | Services refusent les opérations nécessitant un workflow (retour HTTP 503 métier) |
| Reprise | Redémarrage Temporal → workflows reprennent exactement là où ils étaient (déterministe) |
| RPO | 0 (stockage Yugabyte backend) |

### 17.6 SPIRE agent down

| Aspect | Comportement |
|--------|--------------|
| Impact | Renouvellement SVID bloqué |
| Détection | `spire_svid_ttl_seconds < 900` |
| Mode dégradé | SVID valide encore pour la durée TTL (typ. 1h) — fenêtre de grâce |
| Reprise | Relancer agent SPIRE, SVID renouvelé |
| RPO | N/A |

### 17.7 Vault down

| Aspect | Comportement |
|--------|--------------|
| Impact | Rotation secrets, renouvellement cert PKI bloqués |
| Détection | `vault_up == 0` |
| Mode dégradé | Secrets en cache (sidecar) valides jusqu'à TTL |
| Reprise | Unseal Vault (3 opérateurs), consul-template reprend |
| RPO | 0 (Raft Integrated Storage ou backend Yugabyte) |

### 17.8 ARMAGEDDON edge down

| Aspect | Comportement |
|--------|--------------|
| Impact | Trafic Nord-Sud coupé |
| Détection | `armageddon_up == 0` |
| Mode dégradé | Deuxième instance ARMAGEDDON (HA active-active derrière IP flottante) prend le trafic |
| Reprise | Redémarrage Pingora, xDS Controller (port 18000) repousse config |
| RPO | N/A |

### 17.9 Config Resilience4j (Java 21 Spring Boot)

```yaml
resilience4j:
  circuitbreaker:
    instances:
      redpanda-produce:
        slidingWindowSize: 50
        failureRateThreshold: 50
        waitDurationInOpenState: 5s
        permittedNumberOfCallsInHalfOpenState: 5
      kaya-write:
        slidingWindowSize: 100
        failureRateThreshold: 30
        waitDurationInOpenState: 2s
      yugabyte-write:
        slidingWindowSize: 50
        failureRateThreshold: 40
        waitDurationInOpenState: 5s
  retry:
    instances:
      redpanda-produce:
        maxAttempts: 5
        waitDuration: 200ms
        exponentialBackoffMultiplier: 2
  bulkhead:
    instances:
      ingestion-rs-call:
        maxConcurrentCalls: 128
```

### 17.10 Fallbacks par service

| Service | Fallback |
|---------|----------|
| `demande-ms` | Retour HTTP 503 métier + message "service temporairement indisponible, réessayez dans 30 s" |
| `validation-acte-ms` | File d'attente locale (Kafka topic `*.validation.retry.v1`), reprise sur santé restaurée |
| `notify-ms` | Retry exponentiel, bascule SMS → email → push |
| `cert-render-ms` | Retry avec backoff, DLQ `*.pdf.failed.v1`, reprise manuelle |

### 17.11 Matrice dégradation consolidée

| Composant down | Lecture | Écriture | Durée fenêtre dégradée | Perte données | Mitigation |
|----------------|---------|----------|------------------------|---------------|------------|
| KAYA primaire | OK (réplica) | **Bloquée** | ≤ 5 min (failover) | ≤ 1 s | Réplica promu auto |
| KAYA réplica | OK | OK | ∞ (tolérée) | 0 | Reconstitution async |
| Redpanda 1 broker | OK | OK | ∞ | 0 | RF=3 dégradé à 2 |
| Redpanda 2 brokers | OK (stale) | **Bloquée** | ≤ 10 min | 0 | Outbox PENDING accumule |
| Yugabyte 1 TServer | OK | OK | ∞ | 0 | RAFT re-leader |
| Yugabyte 2 TServers | OK (stale) | **Bloquée** | ≤ 15 min | 0 | Restore nécessaire |
| Temporal | Dégradée | Partielle | jusqu'à restart | 0 | Workflows en pause |
| outbox-relay tous workers | OK | Backlog grandit | ≤ 2 min | 0 | K8s relance auto |
| ARMAGEDDON instance 1 | OK (autre) | OK | 0 (bascule IP) | 0 | HA actif-actif |
| Vault | OK (cache secrets) | OK | jusqu'à expiration TTL | 0 | Unseal manuel |
| SPIRE agent | OK | OK | TTL SVID (1h) | 0 | Relance agent |

### 17.12 Runbook : perte KAYA primaire

```shell
# 1. Vérifier santé réplica
kaya-cli -h kaya-replica -p 6381 INFO replication
# → attendu : role:replica, master_link_status:up ou down (si perte réseau)

# 2. Vérifier lag avant promotion
kaya-cli -h kaya-replica -p 6381 INFO replication | grep master_last_io_seconds_ago
# → doit être < 2 s

# 3. Promotion
kaya-cli -h kaya-replica -p 6381 REPLICAOF NO ONE

# 4. Reconfigurer DNS/VIP
kubectl patch svc kaya-primary -p '{"spec":{"selector":{"role":"replica"}}}'

# 5. Vérifier connectivité services
for svc in etat-civil hospital eticket; do
  kubectl exec -n $svc ingestion-rs-0 -- kaya-cli -h kaya-primary PING
done

# 6. Reconstruire ancien primaire en réplica (quand récupéré)
kaya-cli -h kaya-old-primary REPLICAOF kaya-new-primary 6380
```

### 17.13 Runbook : DLQ outbox

Voir §14.5. Synthèse opérationnelle :

```shell
# Nombre messages DLQ par topic
ysqlsh -c "SELECT topic, error_reason, COUNT(*) FROM outbox
           WHERE status='DEAD_LETTER' GROUP BY topic, error_reason
           ORDER BY 3 DESC;"

# Réactivation après correction (exemple ciblé)
ysqlsh -c "UPDATE outbox SET status='PENDING', retry_count=0, error_reason=NULL
           WHERE status='DEAD_LETTER'
             AND topic='etat-civil.pdf.requested.v1'
             AND created_at > '2026-04-15T00:00:00Z';"
```

---

## 18. Dimensionnement — soutenu / pic / burst

### 18.1 Tableau par sous-projet (événements/minute)

| Sous-projet | Soutenu | Pic | Burst | Fenêtre burst |
|-------------|---------|-----|-------|---------------|
| ÉTAT-CIVIL | 80 | 500 | 2 000 | 5 min (rush ouverture mairies) |
| HOSPITAL | 200 | 1 000 | 5 000 | 10 min (urgences) |
| E-TICKET | 500 | 2 000 | 10 000 | 15 min (ouverture ventes) |
| VOUCHERS | 100 | 500 | 3 000 | 5 min (lancement campagne) |
| SOGESY | 300 | 2 000 | 8 000 | 10 min (paiements scolaire / salaires) |
| E-SCHOOL | 50 | 800 | 5 000 | 30 min (rentrée) |
| ALT-MISSION | 10 | 50 | 200 | 5 min (fin de mois) |
| FASO-KALAN | 20 | 100 | 500 | 15 min (leçon synchrone) |
| **Total consolidé** | **1 260** | **6 950** | **33 700** | — |

### 18.2 Capacité Redpanda (NVMe RF=3)

Sur socle matériel **Scale-a7 AMD EPYC 9005** (à valider en bench final) :

| Métrique | Objectif soutenu | Marge |
|----------|------------------|-------|
| Produce (RF=3, acks=all) | ≥ 200 000 msg/s | × 6 vs burst total |
| Latence produce P99 | ≤ 15 ms | 2× marge |
| Consume fan-out | ≥ 500 MB/s | — |
| Tiered offload | ≥ 500 MB/s | — |

> **NOTE.** Le chiffrage Redpanda ci-dessus est **à valider** sur plateforme Scale-a7 AMD EPYC 9005 avant mise en production. Le dimensionnement définitif fera l'objet du rapport `bench-redpanda-scale-a7-YYYY.md` signé par le CAS.

### 18.3 Capacité KAYA

Sur 4 proactor_threads (4 cœurs dédiés), KAYA soutient ≥ 500 000 ops/s single-shard. Les 8 sous-projets partagent 2 clusters KAYA logiques (tenant-isolated par ACL), capacité totale estimée : 1 M ops/s cluster.

### 18.4 Capacité YugabyteDB

Cluster 3 nœuds TServer + 3 Master, RF=3, NVMe. Capacité cible : 50 000 TPS mixtes (90 % lecture / 10 % écriture). Capacité lecture augmentable par ajout de replica read-only (lectures éventuellement consistantes via tablet follower reads).

### 18.5 Dimensionnement matériel par composant

| Composant | CPU | RAM | Disque | Réseau | Nœuds |
|-----------|-----|-----|--------|--------|-------|
| KAYA primary | 16 vCPU | 64 GB | 1 TB NVMe (AOF + snapshots) | 25 Gbps | 1 + 1 réplica |
| Redpanda broker | 24 vCPU | 96 GB | 4 TB NVMe | 25 Gbps | 3 |
| YugabyteDB TServer | 32 vCPU | 128 GB | 4 TB NVMe | 25 Gbps | 3 |
| YugabyteDB Master | 4 vCPU | 16 GB | 200 GB SSD | 10 Gbps | 3 |
| Temporal | 8 vCPU | 32 GB | 200 GB SSD | 10 Gbps | 3 (frontend/history/matching) |
| ARMAGEDDON | 8 vCPU | 16 GB | 100 GB SSD | 25 Gbps | 2 (HA) |
| xDS Controller | 2 vCPU | 4 GB | 20 GB SSD | 1 Gbps | 2 (HA) |
| ingestion-rs | 4 vCPU | 8 GB | 20 GB SSD | 10 Gbps | 3-6 (HPA) |
| outbox-relay | 2 vCPU | 4 GB | 10 GB SSD | 1 Gbps | 6 (3 × 2 HA) |
| pdf-worker-ms | 4 vCPU | 8 GB | 20 GB SSD | 1 Gbps | 4-8 (HPA) |
| Vault | 4 vCPU | 8 GB | 100 GB SSD | 1 Gbps | 3 + 1 HSM |
| Prometheus/Grafana | 8 vCPU | 32 GB | 2 TB SSD (rétention métriques 90 j) | 10 Gbps | 2 |

### 18.6 Scaling strategies

- **HPA CPU > 70 %** pour ingestion-rs, services Java, pdf-worker-ms.
- **HPA custom metrics** pour outbox-relay : si `outbox_relay_pending_count > 100` → +1 pod.
- **Cluster Autoscaler** Kubernetes actif (pool noeuds compute EPYC 9005).
- **Sharding Yugabyte** : tablets sharding automatique selon charge.
- **Partitionnement Redpanda** : 12 partitions par topic critique pour absorber bursts.

---

## 19. Licences et stratégies de sortie

### 19.1 Tableau des licences

| Composant | Licence | Statut | Stratégie de sortie |
|-----------|---------|--------|---------------------|
| **KAYA** | **Souverain FASO DIGITALISATION** (licence propriétaire interne BF) | Contrôle total | N/A (sourced in-house) |
| **ARMAGEDDON** | **Souverain FASO DIGITALISATION** | Contrôle total | N/A |
| **xDS Controller** | **Souverain** | Contrôle total | N/A |
| **Redpanda** | BSL 1.1 (conversion Apache 2.0 après 4 ans) | Fallback prévu | **Apache Kafka + Strimzi** (Kubernetes operator Apache 2.0) ; schémas Protobuf portables tels quels |
| **YugabyteDB** | Apache 2.0 (+ YBLA pour extras) | OSS pur | Fallback **CockroachDB** (BSL, compatible PostgreSQL wire) |
| **Temporal** | MIT | OSS pur | Fallback **Cadence** (Uber, MIT) — fork d'origine, API proche |
| **Prometheus** | Apache 2.0 | OSS pur | N/A (dominant, pas de risque) |
| **Grafana** | AGPL v3 (OSS) / Grafana Enterprise (commerciale) | OSS utilisé | N/A |
| **Jaeger** | Apache 2.0 (CNCF graduated) | OSS pur | Fallback **Tempo** (Grafana Labs, AGPL) ou **SigNoz** |
| **SPIRE** | Apache 2.0 (CNCF) | OSS pur | N/A |
| **Vault** | BSL 1.1 (HashiCorp) | Fallback prévu | **OpenBao** (fork LF, MPL 2.0) — drop-in compatible API Vault |
| **Rhai** | MIT / Apache 2.0 | OSS pur | N/A (embedded Rust) |
| **Typst** | Apache 2.0 | OSS pur | N/A |

### 19.2 Exercice annuel de sortie

Chaque année, le CAS exécute un **drill de sortie** pour les composants BSL :

- **Redpanda → Kafka/Strimzi** : déploiement d'un cluster Kafka 3 brokers en environnement de test, reproduction des topics critiques, vérification compatibilité Kafka API côté producers Java.
- **Vault → OpenBao** : drop-in en staging, test tous les secret engines utilisés (KV v2, PKI, Transit).

Rapports de drill versionnés dans `dr-exit-drills/YYYY/`.

### 19.3 Clause souveraine

Tout nouveau composant introduit dans l'écosystème doit être :

1. soit **souverain** (KAYA, ARMAGEDDON, xDS Controller, pipelines Rust internes) ;
2. soit **OSS permissif** (Apache 2.0, MIT, BSD, MPL 2.0) ;
3. soit **OSS avec stratégie de sortie documentée** (cas Redpanda, Vault) validée par le CAS.

Aucune licence purement propriétaire étrangère n'est admise dans le chemin critique.

### 19.4 Audit annuel des dépendances

Un script `dep-audit` tourne quotidiennement en CI :

- **SBOM généré** (CycloneDX) pour chaque image Docker et binaire Rust.
- Scan licences via **FOSSA** (on-prem) ou **syft + grype** (OSS).
- Alerte si nouvelle dépendance transitivementsous licence **incompatible** (GPL v3 non-wrapper, AGPL si non-service, SSPL, etc.).

### 19.5 Plan de sortie Redpanda → Kafka (simulation)

| Étape | Durée estimée | Livrable |
|-------|---------------|----------|
| 1. Déploiement Kafka 3.x en parallèle | 5 j | Cluster Kafka 3 brokers |
| 2. Mirror Maker 2 Redpanda → Kafka | 3 j | Topics miroirs temps réel |
| 3. Bascule producers | 2 j | Prod écrit dans Kafka, Redpanda legacy |
| 4. Bascule consumers | 3 j | Toutes apps sur Kafka |
| 5. Décommissionnement Redpanda | 1 j | Retrait |
| **Total** | **14 j** | — |

### 19.6 Plan de sortie Vault → OpenBao

Compatibilité API Vault 1.14 ≈ OpenBao 2.0. Migration en 3 étapes :

1. Export via `vault operator raft snapshot save`.
2. Import dans OpenBao via `bao operator raft snapshot restore`.
3. Re-provisionnement progressif des sidecars via rolling update K8s.

Durée estimée : 7 jours, sans downtime pour les consumers (cache TTL secrets couvre la bascule).

---

## 20. Anti-patterns interdits

> **CRITIQUE.** Les patterns listés ci-dessous sont **proscrits**. Leur détection en revue de code ou en audit entraîne un rejet immédiat et la réécriture obligatoire.

### 20.1 Anti-patterns de cohérence

1. **Écrire dans Redpanda avant KAYA.** Viole la règle d'or. Ouvre une fenêtre où un consumer réagit à un événement dont le hot path n'a pas connaissance.
2. **Écrire dans YugabyteDB hors `ingestion-rs`.** Crée des écritures silencieuses qui échappent à l'outbox et donc au journal légal.
3. **Utiliser KAYA comme source légale.** KAYA n'a pas de RF=3 RAFT. Toute utilisation en preuve juridique est non opposable.
4. **Utiliser Redpanda comme base transactionnelle synchrone.** Latence > 10 ms et pas de requêtes relationnelles.
5. **Oublier l'idempotency_key.** Ouvre la porte aux duplications sur retry.

### 20.2 Anti-patterns d'atomicité

6. **WATCH/CAS sur clé très chaude.** Thrash garanti, débit en chute libre. Remplacer par Rhai.
7. **EVAL (source brute) en production.** Désactivé par ACL. Toujours `EVALSHA <sha>`.
8. **Scripts Rhai non versionnés Git.** Non auditable. Tous les scripts vivent dans `kaya-rhai-scripts/`.
9. **MULTI/EXEC multi-shard sans retry.** OCC VLL peut abort. Sans retry, échecs silencieux.
10. **Pipeline pour des opérations dépendantes.** Pas d'atomicité : une commande peut échouer sans rollback.

### 20.3 Anti-patterns de durabilité

11. **KAYA `--persistence no` en production.** Interdit. Alerte P1 CRITIQUE.
12. **`fsync no`.** Interdit.
13. **Redpanda `acks=1` ou `acks=0`.** Ouvre la porte à la perte. Toujours `acks=all` + `min.insync.replicas=2`.
14. **Oublier `SET FOR UPDATE SKIP LOCKED` dans outbox-relay.** Duplications garanties entre workers.

### 20.4 Anti-patterns de sécurité

15. **PII en clair dans Redpanda.** Violation RGPD/loi 010-2004/AN.
16. **ACL KAYA `+@all`.** Désactive tout l'isolement par tenant.
17. **Secret statique hard-codé.** Tous les secrets passent par Vault + SVID SPIRE.
18. **mTLS désactivé en Est-Ouest.** Aucune exception.
19. **`-keys`, `-flushall`, `-flushdb` non retirés.** Risque de flush accidentel.

### 20.5 Anti-patterns d'observabilité

20. **PII en clair dans les logs.** Filtrer en amont (vector).
21. **Métriques cardinality-explosion** (tag `user_id` sur un compteur). Utiliser hash/bucket.
22. **Alerte sans runbook.** Toute alerte doit pointer vers un runbook documenté.

### 20.6 Anti-patterns d'évolution de schéma

23. **Renommer un champ Protobuf.** Créer un nouveau champ + `reserved`.
24. **Changer le tag Protobuf.** Interdit.
25. **Publier un schéma en `FULL_TRANSITIVE` là où `BACKWARD_TRANSITIVE` suffit.** Bloque l'évolution sans gain.

### 20.7 Anti-patterns opérationnels

26. **Déployer sans ADR** pour un changement d'architecture.
27. **Promouvoir un réplica KAYA sans vérifier le lag.** Perte > 1 s possible.
28. **Rejouer un topic `*.pii-access.v1` vers un topic normal.** Rétention différente, traçabilité perdue.
29. **Exécuter un chaos scenario en production non annoncé.** Tous les drills sont planifiés.
30. **Ignorer un message DLQ > 48 h.** Obligation de traitement sous 48 h.

### 20.8 Anti-patterns de déploiement

31. **Déployer un script Rhai sans test unitaire vert.** La CI doit valider.
32. **Déployer un schéma Protobuf sans `buf breaking` vert.** Risque de casser des consumers.
33. **Utiliser `kubectl edit` en production.** Tout changement passe par GitOps (ArgoCD).
34. **Scaling à chaud de YugabyteDB sans vérifier les tablets balancing.** Peut induire déséquilibre et dégradation.
35. **Déployer ARMAGEDDON sans validation xDS.** Le xDS Controller doit valider la config avant push.

### 20.9 Anti-patterns de données

36. **Dupliquer la vérité entre KAYA et Yugabyte.** KAYA doit projeter depuis Yugabyte, jamais l'inverse.
37. **Stocker des timestamps en string.** Toujours Unix epoch ms ou `TIMESTAMPTZ`.
38. **Utiliser `SERIAL` ou `BIGSERIAL` pour un ID public.** Prédictible → exploitable. Utiliser UUID v4 ou v7.
39. **Oublier `NOT NULL` + `DEFAULT` sur les colonnes d'état.** Bug silencieux.
40. **Ne pas partitionner une table > 10 Go.** Performance dégradée.

### 20.10 Anti-patterns de clients

41. **Client HTTP sans timeout.** Obligatoire de fixer un timeout (typ. 5 s API externe).
42. **Client gRPC sans deadline propagée.** Chain breakage.
43. **Retry infini.** Toujours avec maxAttempts + backoff exponentiel.
44. **Cacher indéfiniment sans TTL.** Fuite mémoire garantie.
45. **Utiliser un `@Cacheable` Spring sans cache key explicite.** Collisions possibles.

---

## Annexes

### A. Glossaire

- **ADR** : Architecture Decision Record
- **CAS** : Comité d'Architecture Souveraine
- **DLQ** : Dead Letter Queue
- **HMAC** : Hash-based Message Authentication Code
- **OCC VLL** : Optimistic Concurrency Control with Value Locking Layer
- **RPO/RTO** : Recovery Point Objective / Recovery Time Objective
- **SVID** : SPIFFE Verifiable Identity Document
- **TDE** : Transparent Data Encryption
- **WORM** : Write-Once Read-Many

### B. Références

- Loi 010-2004/AN du Burkina Faso (protection des données à caractère personnel)
- Règlement CNIL-BF 2021-014 (secret médical numérique)
- BCEAO — Directives sur les systèmes de paiement électronique
- SPIFFE/SPIRE : https://spiffe.io
- RAFT Consensus Algorithm (Ongaro, Ousterhout, 2014)
- Protobuf BACKWARD_TRANSITIVE : https://docs.confluent.io/platform/current/schema-registry/avro.html

### C. Tableau récapitulatif des ports et endpoints

| Composant | Port(s) | Protocole | TLS | mTLS |
|-----------|---------|-----------|-----|------|
| ARMAGEDDON | 443 (public), 8080 (metrics) | HTTP/3, HTTP/2 | Oui | — (edge) |
| xDS Controller | 18000 | gRPC | Oui | Oui |
| KAYA primaire | 6380 | RESP3+ | Oui | Oui |
| KAYA réplica / gRPC | 6381 | RESP3+ / gRPC | Oui | Oui |
| Redpanda Kafka API | 9092, 9093 (internal) | Kafka binary | Oui | Oui (SASL SPIFFE) |
| Redpanda Admin API | 9644 | HTTP | Oui | Oui |
| Redpanda Schema Registry | 8081 | HTTP | Oui | Oui |
| YugabyteDB YSQL | 5433 | PostgreSQL wire | Oui | Oui |
| YugabyteDB Master UI | 7000 | HTTP | Oui | — |
| YugabyteDB TServer | 9000, 9100 | gRPC | Oui | Oui |
| Temporal frontend | 7233 | gRPC | Oui | Oui |
| SPIRE Server | 8081 | gRPC | Oui | Oui |
| Vault | 8200 | HTTP | Oui | — |
| Prometheus | 9090 | HTTP | Oui | — |
| Grafana | 3000 | HTTP | Oui | — |
| Jaeger collector | 14250 (gRPC), 14268 (HTTP) | gRPC/HTTP | Oui | Oui |
| ingestion-rs gRPC | 50051 | gRPC | Oui | Oui |
| outbox-relay metrics | 9101 | HTTP | — | — |

### D. Matrice responsabilités (RACI condensé)

| Activité | Dev | SRE | CAS | Security | Métier |
|----------|:---:|:---:|:---:|:--------:|:------:|
| Ajout topic Redpanda | R | A/C | C | C | I |
| Évolution schéma Protobuf | R | C | A | I | I |
| Ajout script Rhai | R | C | A | C | I |
| Modification ACL KAYA | — | R | A | A/C | I |
| Politique OPA | C | R | A | R | I |
| Runbook incident P1 | — | R | A | C | I |
| Rotation clé HMAC SOGESY | — | R | A | R | I |
| Revue SLO trimestrielle | I | R | A | I | C |
| Chaos drill | I | R | A | C | I |
| Droit à l'oubli RGPD | C | R | A | A | R |

Légende : R = Responsable d'exécution, A = Autorité d'approbation, C = Consulté, I = Informé.

### E. Catalogue complet des microservices par projet

#### ÉTAT-CIVIL (11 microservices)

| Service | Langue | Rôle |
|---------|--------|------|
| `ingestion-rs` | Rust | Écritures centralisées |
| `demande-ms` | Java 21 | API GraphQL demandes |
| `traitement-acte-ms` | Java 21 | Workflow traitement |
| `validation-acte-ms` | Java 21 | Four-eyes validation |
| `signature-ms` | Java 21 | XAdES officier |
| `impression-ms` | Java 21 | Demande PDF |
| `notify-ms` | Java 21 | SMS / email |
| `audit-ms` | Java 21 | Miroir audit |
| `cert-render-ms` | Rust | Rendu Typst |
| `outbox-relay` | Rust | Dispatcher outbox |
| `reconciliator-rs` | Rust | Rapprochements |

#### HOSPITAL (13 microservices)

Ajoute : `dossier-medical-ms`, `prescription-ms`, `pharmacie-ms`.

#### E-TICKET (9 microservices)

`billetterie-ms`, `checkin-ms`, `reservation-ms`, `paiement-ticket-ms`, plus socle commun.

#### VOUCHERS (10 microservices)

`emission-ms`, `consommation-ms`, `campagne-ms`.

#### SOGESY (14 microservices)

`paiement-ms`, `rapprochement-ms`, `chaine-hmac-ms`, `bceao-reporting-ms`.

#### E-SCHOOL (12 microservices)

`inscription-ms`, `emploi-du-temps-ms`, `notes-ms`, `pedagogie-ms`.

#### ALT-MISSION (8 microservices)

`mission-ms`, `indemnite-ms`, `rapport-mission-ms`.

#### FASO-KALAN (10 microservices)

`lecon-ms`, `progression-ms`, `evaluation-ms`.

**Total plateformes communes (Auth/BFF/Admin)** : 20 microservices transverses (authentication, tenant-management, notification-hub, file-store, search-index, audit-hub, etc.).

**Grand total** : 107 microservices.

### F. Exemple ADR

```markdown
# ADR-012 : Passage de Lua à Rhai pour les scripts KAYA

## Statut
Accepté — 2026-03-12

## Contexte
Le guide v3.0 utilisait des scripts Lua dans DragonflyDB. Avec la bascule souveraine vers KAYA (Rust),
la question du langage de scripting se pose : garder Lua (compatibilité historique) ou
adopter Rhai (natif Rust, meilleure intégration) ?

## Décision
Adopter **Rhai 5.4** comme seul langage de scripting serveur-side dans KAYA.

## Conséquences
- Réécriture de 6 scripts historiques (coût ~2 semaines).
- Meilleure performance (Rhai 5.4 avec auto-async est 2× plus rapide que Lua embarqué).
- Sandboxing plus fort.
- Perte de compatibilité avec la bibliothèque Lua communautaire Redis (acceptable, contexte souverain).

## Alternatives écartées
- **Lua** : sous-optimal, ne tire pas parti du moteur Rust natif.
- **WASM** : surdimensionné pour nos besoins, complexité de chargement.
- **JavaScript (V8)** : empreinte mémoire trop élevée.
```

### G. Changelog v3.0 → v3.1

- Remplacement intégral **DragonflyDB → KAYA** (Rust, ports 6380/6381, RESP3+/gRPC).
- Remplacement intégral **Lua → Rhai 5.4**, avec `rhai_auto_async=true`.
- Remplacement intégral **Envoy → ARMAGEDDON** (Pingora, SENTINEL/ARBITER/ORACLE/AEGIS/NEXUS, WAF Coraza, jwt_authn ES384, ext_authz OPA).
- Ajout du **xDS Controller souverain** (port 18000).
- Ajout section dédiée **chaîne HMAC SOGESY** via `hmac_chain_advance.rhai`.
- Précision **rétentions topics** (10 ans audit/hospital/sogesy/etat-civil, 7 ans vouchers, 5 ans e-school/alt-mission, 2 ans faso-kalan, 90 j e-ticket).
- Matrices **RPO et RTO par sous-projet** complétées.
- Durcissement **anti-patterns** : 30 entrées (vs 18 en v3.0).
- Clause **stratégie de sortie** mise à jour : Vault → OpenBao.

---

*Fin du document. Toute dérogation aux règles énoncées ci-dessus exige un ADR validé par le Comité d'Architecture Souveraine.*
