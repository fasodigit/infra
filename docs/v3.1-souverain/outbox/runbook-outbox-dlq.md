# Runbook on-call — Outbox Dead-Letter Queue

**Service** : `outbox-relay`
**Severity alert** : `OutboxDeadLetter` (critical, page on-call)
**SLA** : diagnostic + mitigation < 15 min
**Rétention ticket** : 10 ans (compliance souveraineté)
**Dernière mise à jour** : 2026-04-16

---

## 0. Pré-requis on-call

| Accès                       | Comment l'obtenir                                          |
|-----------------------------|------------------------------------------------------------|
| VPN FASO DEVOPS             | `sudo wg-quick up faso-devops`                             |
| `psql` YugabyteDB           | rôle `outbox_admin` → secret vault `kv/outbox/<module>`    |
| Grafana                     | https://grafana.faso.bf                                    |
| Dashboard Outbox            | https://grafana.faso.bf/d/outbox-relay/overview            |
| Logs (Loki)                 | https://grafana.faso.bf/explore?datasource=loki            |
| Redpanda console            | https://redpanda-console.faso.bf                           |
| kubectl                     | contexte `faso-prod`                                       |

Définir le module concerné (visible dans l'alerte Alertmanager) :

```bash
export MODULE="etat-civil"     # ou hospital / e-ticket / vouchers / ...
export YB_DSN="postgres://outbox_admin@yb-prod-0.faso.bf:5433/${MODULE//-/_}"
```

---

## Étape 1 — Inventaire DEAD_LETTER

Connexion et lecture des 50 dernières lignes en DLQ :

```bash
psql "$YB_DSN" <<'SQL'
\x on
SELECT
    id,
    aggregate_id,
    aggregate_type,
    event_type,
    retry_count,
    error_reason,
    created_at,
    updated_at
FROM outbox
WHERE status = 'DEAD_LETTER'
ORDER BY updated_at DESC
LIMIT 50;
SQL
```

Comptage rapide par cause :

```sql
SELECT
    substring(error_reason from 1 for 80) AS reason,
    COUNT(*) AS nb,
    MIN(updated_at) AS first_seen,
    MAX(updated_at) AS last_seen
FROM outbox
WHERE status = 'DEAD_LETTER'
  AND updated_at > NOW() - INTERVAL '1 hour'
GROUP BY reason
ORDER BY nb DESC;
```

Si **nb > 1000** ou **types d'événements variés** → c'est probablement une panne d'infrastructure, pas un bug métier. Escalader §3.

---

## Étape 2 — Analyse du `error_reason`

Classification :

| Motif `error_reason`                                       | Cause probable              | §    |
|------------------------------------------------------------|-----------------------------|------|
| `timeout` / `Broker transport failure`                     | Redpanda down / lent        | 3.1  |
| `TopicAuthorizationFailed`                                 | ACL Redpanda révoquée       | 3.2  |
| `UnknownTopicOrPartition`                                  | Topic non créé              | 3.3  |
| `InvalidRecord` / `schema ... not found`                   | Schema Registry refuse      | 3.4  |
| `KAYA XADD timeout`                                        | KAYA saturé (non bloquant seul) | 3.5  |
| `connection refused: postgres`                             | Worker YugabyteDB KO        | 3.6  |
| `fatal: signature SPIFFE expired`                          | Rotation SVID échouée       | 3.7  |

---

## Étape 3 — Diagnostic root cause

### 3.1 Redpanda down ou lent

- Dashboard : **Grafana → Redpanda Cluster Health**
- Panneau `kafka.brokers.up` : doit valoir 3/3
- Panneau `produce.latency.p99` : doit rester < 50 ms

```bash
kubectl -n redpanda get pods -l app=redpanda
kubectl -n redpanda logs -l app=redpanda --tail=200 | grep -iE "error|panic|raft"
```

Fix : redémarrer le broker défaillant (`kubectl delete pod redpanda-X`), vérifier reprise RAFT.

### 3.2 ACL Redpanda révoquée

```bash
rpk acl list --user outbox-relay-${MODULE}
```

Attendu : `produce` sur `${MODULE}.events.v*`. Sinon, rejouer le manifeste `infra/redpanda/acl-outbox.yaml` :

```bash
kubectl apply -f INFRA/redpanda/acl-outbox.yaml
```

### 3.3 Topic manquant

```bash
rpk topic describe "${MODULE}.events.v1"
```

Si inexistant :

```bash
rpk topic create "${MODULE}.events.v1" \
    --partitions 12 --replicas 3 \
    --config retention.ms=63072000000  # 2 ans
```

### 3.4 Schema Registry refuse

Dashboard : **Grafana → Schema Registry → rejections**

```bash
curl -s https://schema.faso.bf/subjects/${MODULE}.events.v1-value/versions/latest | jq
```

Comparer au schéma envoyé (logs `outbox-relay` avec `event_id` de l'alerte). Soit enregistrer la nouvelle version compatible, soit corriger le writer `ingestion-rs`.

### 3.5 KAYA saturé

**Note** : seul KAYA down ne doit PAS provoquer de DEAD_LETTER (SPEC §13.1). Si ça arrive → bug à remonter équipe core.

### 3.6 YugabyteDB

```bash
kubectl -n yugabyte get pods
psql "$YB_DSN" -c "SELECT 1;"
```

### 3.7 SVID expiré

```bash
kubectl -n outbox exec deploy/outbox-relay-${MODULE} -- \
    spire-agent api fetch x509 -socketPath /run/spire/sockets/agent.sock
```

Expected TTL > 30 min. Sinon redémarrer SPIRE agent :

```bash
kubectl -n spire rollout restart ds/spire-agent
```

---

## Étape 4 — Mitigation : relance des lignes DEAD_LETTER

**Condition** : cause identifiée ET corrigée (§3). Sinon on recrée le problème.

### 4.1 Relance ciblée (recommandée)

```sql
-- Dry-run : lister ce qu'on va relancer
SELECT id, event_type, aggregate_id, created_at
  FROM outbox
 WHERE status = 'DEAD_LETTER'
   AND error_reason ILIKE '%Broker transport failure%'
   AND updated_at > NOW() - INTERVAL '2 hours';

-- Relance
BEGIN;
UPDATE outbox
   SET status = 'PENDING',
       retry_count = 0,
       error_reason = NULL,
       updated_at = NOW()
 WHERE status = 'DEAD_LETTER'
   AND error_reason ILIKE '%Broker transport failure%'
   AND updated_at > NOW() - INTERVAL '2 hours';
-- Vérifier nombre de lignes, puis :
COMMIT;
```

### 4.2 Relance par lot d'IDs (chirurgicale)

```sql
UPDATE outbox
   SET status = 'PENDING',
       retry_count = 0,
       error_reason = NULL,
       updated_at = NOW()
 WHERE id IN (
    'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
    'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb'
 );
```

### 4.3 Interdiction

NE JAMAIS faire `UPDATE outbox SET status='PENDING' WHERE status='DEAD_LETTER';` sans filtre : si la cause racine n'est pas 100% corrigée, on re-saturrera les workers.

---

## Étape 5 — Vérification de reprise

Attendre 60 s puis vérifier trois indicateurs :

1. **`outbox_relay_pending_count` décroît** (Grafana panneau "Pending backlog")
2. **`outbox_relay_sent_total` augmente** (Grafana panneau "Sent rate")
3. **Pas de nouveau DEAD_LETTER** sur la dernière minute :

```sql
SELECT COUNT(*) FROM outbox
 WHERE status = 'DEAD_LETTER'
   AND updated_at > NOW() - INTERVAL '1 minute';
```

Doit retourner 0 (hors lignes pré-existantes).

Si le backlog ne descend pas :
- worker bloqué ? `kubectl logs deploy/outbox-relay-${MODULE}` sur les 2 instances
- lock stale YugabyteDB ? `SELECT pid, query, state FROM pg_stat_activity WHERE query LIKE '%outbox%';`

---

## Étape 6 — Post-mortem

Créer un ticket **dans le tracker compliance** (rétention 10 ans) :

- Titre : `[Outbox] DLQ ${MODULE} — <date> — <cause courte>`
- Champs obligatoires :
    - Début/fin incident (timestamps ISO-8601 UTC)
    - Nombre d'événements impactés (comptés § étape 1)
    - Root cause confirmée
    - Correctif appliqué (commande exacte + commit SHA si code)
    - Actions préventives (règle Prometheus à ajuster, test chaos à automatiser, etc.)
    - Lien Grafana snapshot (bouton "Share > Snapshot" pendant l'incident)
- Validation : lead SRE + lead module métier

Template minimal :

```
## Contexte
Service : outbox-relay/${MODULE}
Alerte  : OutboxDeadLetter (critical)
Début   : 2026-XX-XX HH:MM UTC
Fin     : 2026-XX-XX HH:MM UTC
Durée   : XX min
Impact  : XXX événements en DLQ, XX clients affectés

## Root cause
<texte>

## Timeline
- HH:MM — alerte reçue
- HH:MM — diagnostic §3.X
- HH:MM — correctif appliqué
- HH:MM — reprise confirmée

## Actions
- [ ] …
- [ ] …
```

---

## Annexes

### A. Requêtes SQL utilitaires

```sql
-- Snapshot rapide
SELECT * FROM outbox_status_summary;

-- Top 10 agrégats en retard
SELECT aggregate_id, COUNT(*)
  FROM outbox
 WHERE status = 'PENDING'
 GROUP BY aggregate_id
 ORDER BY 2 DESC
 LIMIT 10;

-- Distribution des âges PENDING
SELECT
    width_bucket(EXTRACT(EPOCH FROM (NOW() - created_at)), 0, 600, 10) AS bucket,
    COUNT(*)
  FROM outbox
 WHERE status = 'PENDING'
 GROUP BY bucket
 ORDER BY bucket;
```

### B. Endpoints Grafana

- Overview : https://grafana.faso.bf/d/outbox-relay/overview
- Deep dive per shard : https://grafana.faso.bf/d/outbox-relay/shards
- Redpanda : https://grafana.faso.bf/d/redpanda/cluster-health
- YugabyteDB : https://grafana.faso.bf/d/yugabyte/overview

### C. Escalade

| Délai depuis alerte | Action                                              |
|---------------------|------------------------------------------------------|
| 0–15 min            | On-call primaire                                     |
| 15–30 min           | On-call primaire + lead SRE                          |
| 30 min+             | + directeur technique + cellule souveraineté         |
| 1 h+                | Communication publique si SLA externe impacté        |

---

**Fin du runbook**
