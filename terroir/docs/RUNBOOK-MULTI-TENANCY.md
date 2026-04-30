<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# RUNBOOK — Multi-Tenancy TERROIR (schema-per-tenant)

**Date** : 2026-04-30  
**Ref** : ADR-006, ULTRAPLAN §3, P0.3

---

## Architecture

TERROIR utilise **schema-per-tenant** sur un seul cluster PostgreSQL.

Convention de nommage :

| Objet | Pattern | Exemple |
|---|---|---|
| Schema métier | `terroir_t_<slug>` | `terroir_t_uph_hounde` |
| Schema audit | `audit_t_<slug>` | `audit_t_uph_hounde` |
| Bucket MinIO | `terroir-t-<slug>` | `terroir-t-uph_hounde` |

Le `slug` est la clé invariante d'un tenant (3-50 chars, `[a-z0-9_]`).  
Le schema partagé `terroir_shared` contient les tables transverses (cooperative, agent_session, geo_check_cache, indicator_value, mooc_module, input_catalog, account_chart).

---

## Onboarding via terroir-admin :9904

```bash
# Créer un tenant
curl -X POST http://localhost:9904/admin/tenants \
  -H 'Content-Type: application/json' \
  -d '{"slug":"uph_hounde","legal_name":"UPH Houndé","country_iso2":"BF",
       "region":"Hauts-Bassins","primary_crop":"coton"}'

# Vérifier le statut
curl http://localhost:9904/admin/tenants/uph_hounde

# Lister les tenants (paginé)
curl "http://localhost:9904/admin/tenants?limit=50"

# Suspendre
curl -X POST http://localhost:9904/admin/tenants/uph_hounde/suspend
```

Workflow interne (< 5 min SLA) :

1. `INSERT terroir_shared.cooperative` (status=PROVISIONING)  
2. Chargement des templates `T001..T100.sql.tmpl`  
3. Substitution `{{SCHEMA}}`/`{{AUDIT_SCHEMA}}` + exécution séquentielle  
4. `UPDATE cooperative SET status='ACTIVE'`  
5. Publication event `auth.terroir.tenant.provisioned` (Redpanda — P0.5)

---

## Backup per-tenant

```bash
# Dump complet d'un tenant (métier + audit)
pg_dump -h localhost -p 5432 -U postgres \
  --schema=terroir_t_uph_hounde \
  --schema=audit_t_uph_hounde \
  -Fc -f /backups/uph_hounde_$(date +%Y%m%d).dump postgres

# Restore (tenant isolé, sans toucher les autres)
pg_restore -h localhost -p 5432 -U postgres \
  -d postgres --schema=terroir_t_uph_hounde \
  /backups/uph_hounde_20260430.dump
```

Retention recommandée : 30 jours rolling + snapshot mensuel 1 an.

---

## Migrations

**Migrations partagées** (appliquées une seule fois sur la DB) :

```bash
psql -h localhost -p 5432 -U postgres -d postgres \
  -f INFRA/terroir/migrations/V001__shared_extensions.sql
psql ... -f V002__shared_schema.sql
psql ... -f V003__rls_helpers.sql
```

**Templates par tenant** (`migrations/tenant-template/T*.sql.tmpl`) :  
Appliqués automatiquement à chaque `POST /admin/tenants`. Pour ré-appliquer un template manuellement (ex : migration additive) :

```bash
# Substituer manuellement et exécuter
sed 's/{{SCHEMA}}/terroir_t_uph_hounde/g; s/{{AUDIT_SCHEMA}}/audit_t_uph_hounde/g' \
  T005__parts_sociales.sql.tmpl | psql -h localhost -U postgres -d postgres
```

---

## pgbouncer + RLS Pattern Rust

pgbouncer opère en mode `transaction` (no prepared statements). Côté sqlx :

```
DATABASE_URL=postgres://terroir_svc:pass@pgbouncer-terroir:6432/faso_terroir
             ?statement_cache_capacity=0
```

Pattern d'isolation dans une transaction sqlx :

```rust
let mut tx = pool.begin().await?;
// Positionner le tenant courant (SET LOCAL = scope transaction uniquement)
sqlx::query("SET LOCAL app.current_tenant_slug = $1")
    .bind(&tenant_slug)
    .execute(&mut *tx).await?;
sqlx::query("SET LOCAL search_path TO $1, terroir_shared, public")
    .bind(&schema_name)
    .execute(&mut *tx).await?;
// Requêtes métier ici — isolation garantie par schema + SET LOCAL
tx.commit().await?;
```

RLS est activé sur toutes les tables tenant (`ENABLE ROW LEVEL SECURITY`). La policy `_app_all` accorde l'accès complet à `terroir_app` — l'isolation est au niveau schema Postgres, pas au niveau ligne (defense in depth).

---

## Limites et scaling

| Métrique | Seuil alerte | Action |
|---|---|---|
| `SELECT count(*) FROM pg_namespace WHERE nspname LIKE 'terroir_t_%'` | > 10 000 | Plan shard PG horizontal |
| Durée provisioning P95 | > 5 min | Debug templates, index PG |
| Lag migration tenant | > 7 jours | Alarme RSSI + escalade |

**Au-delà de 20k tenants** : évaluer Citus extension (sharding horizontal) ou YugabyteDB (distribué). Décision à la gate P5 (ULTRAPLAN §1, ADR-006 §Révision).

Monitoring `pg_namespace` count :

```sql
SELECT count(*) AS tenant_schema_count
FROM pg_namespace
WHERE nspname LIKE 'terroir_t_%';
```

Ne jamais exécuter `SELECT count(*) FROM terroir_shared.cooperative` sur le chemin chaud — utiliser les index partiels ou les métriques Prometheus via `kaya_wal_io_errors_total` équivalent terroir.
