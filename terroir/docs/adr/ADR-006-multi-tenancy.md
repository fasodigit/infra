<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# ADR-006 — Stratégie de multi-tenancy

| Champ | Valeur |
|---|---|
| Statut | Proposé |
| Date | 2026-04-30 |
| Décideurs | Tech lead, RSSI, BizDev |
| Contexte | TERROIR — isolation entre coopératives, unions, exportateurs, bailleurs |

## Contexte

TERROIR sera utilisé simultanément par :
- Plusieurs **coopératives primaires** (5-200 membres)
- Plusieurs **unions / faîtières** (5-50 coopératives chacune)
- Plusieurs **exportateurs** (ils consomment les DDS de coopératives qu'ils contractualisent)
- Plusieurs **bailleurs** (vue M&E sur projets qu'ils financent — pas tous les producteurs)

Une coopérative ne doit JAMAIS voir les producteurs d'une autre. Un exportateur voit uniquement les coopératives sous contrat. Un bailleur voit uniquement les indicateurs M&E de SES projets.

### Volume cible an 3
- ~50 coopératives clients (= 50 « top tenants »)
- ~5-10 exportateurs
- ~3-5 bailleurs
- ~200 000 producteurs cumulés
- Volume données : ~50 GB/an PG, ~500 GB/an MinIO

### Contraintes
- Conformité données personnelles (segmentation forte exigée)
- Performance : ne pas dégrader les requêtes par scan multi-tenant
- Coût : mutualisation infra obligatoire (50 PG instances séparées = inviable)
- Onboarding rapide : créer une coop = quelques minutes, pas une journée

## Options envisagées

### Option A — Database-per-tenant
**Pour** : isolation maximale, restore tenant-spécifique facile.
**Contre** : 50+ DBs à provisionner, monitorer ; coût ; migrations × N.

### Option B — Schema-per-tenant
**Pour** : isolation très bonne (SQL niveau schema), moins coûteux que A.
**Contre** : migrations à appliquer × N (gérable avec orchestration), connection pool plus subtil.

### Option C — Row-level (tenant_id partout, RLS PostgreSQL)
**Pour** : un seul schema, performance OK, simple à déployer.
**Contre** : risque erreur appli (omission filtre tenant_id) → leak ; RLS peut être contourné si superuser.

### Option D — Hybride : schema-per-tenant pour métier, RLS pour sous-tenants
**Pour** : coopératives clientes top-isolées (schema), unions = entités regroupant coops, exportateurs/bailleurs = lecteurs avec ABAC granulaire.
**Contre** : modèle un peu sophistiqué, mais réaliste pour TERROIR.

### Option E — Cluster Kubernetes par tenant
**Pour** : ultime isolation.
**Contre** : ridicule pour notre échelle.

## Décision

**Option D — Hybride : schema-per-tenant + RLS PostgreSQL + Keto ABAC + JWT claim `tenant_id`.**

### Définition « tenant »
Un **tenant primaire** = une coopérative cliente (entité signant le contrat SaaS).
- 1 schema PG par tenant : `terroir_t_<slug>` (ex: `terroir_t_uph_hounde`)
- 1 bucket MinIO logique par tenant : `terroir-t-<slug>`
- 1 namespace Keto par tenant pour les permissions

Une **union** = méta-tenant qui regroupe N coops :
- Pas de schema dédié (ses données = vues croisant les schemas membres)
- Permissions Keto : utilisateur `union_admin` a `read` sur tous les schemas membres

Un **exportateur** = consommateur :
- Aucun schema en propre, accès cross-tenant via Keto policies (lecture restreinte aux contrats actifs)

Un **bailleur** = consommateur M&E :
- Vues agrégées dans schema dédié `terroir_donor_<slug>` alimentées par CDC Redpanda
- Pas d'accès aux PII brutes (uniquement agrégats anonymisés)

### Architecture

```
┌──────────────────────────────────────────────────┐
│ ARMAGEDDON (front gateway)                       │
│   - JWT validation + claim extraction            │
│   - claim: tenant_id, role, allowed_schemas[]    │
└──────────────┬───────────────────────────────────┘
               ▼
┌──────────────────────────────────────────────────┐
│ terroir-core / terroir-eudr / etc.               │
│  - SET search_path TO terroir_t_<slug>;          │
│  - Connection pool tenant-scoped (pgbouncer)     │
│  - Keto check pour cross-tenant queries          │
└──────────────┬───────────────────────────────────┘
               ▼
┌──────────────────────────────────────────────────┐
│ PostgreSQL                                       │
│  - schema terroir_shared      (catalogue, ref)   │
│  - schema terroir_t_<slug>    (par coop)         │
│  - schema terroir_union_<u>   (vue union)        │
│  - schema terroir_donor_<d>   (vue M&E)          │
│  - RLS sur quelques tables transverses           │
└──────────────────────────────────────────────────┘
```

### Migrations
- Outil : Flyway (multi-schema mode)
- Workflow : `flyway migrate -schemas=terroir_t_<slug>` lancé par CI déploiement
- Test obligatoire : `flyway migrate` sur tenant test + snapshot avant prod
- Rollback : par tenant individuel (pas global)

### Onboarding tenant (provisionning)
1. Création tenant via terroir-admin (port 9904)
2. Migration Flyway sur nouveau schema
3. Bucket MinIO + policies
4. Namespace Keto + relations init
5. JWT claim `tenant_id` injecté côté Kratos session
6. SLA cible : ≤ 5 minutes du clic à coop opérationnelle

### Cross-tenant queries (cas exportateur, bailleur)
- Vue matérialisée `terroir_shared.export_dds_v` rafraîchie par CDC
- Filtrage Keto + tenant_id whitelist par utilisateur
- Audit log every read (Loki) — détection accès anormal

## Conséquences

### Positives
- Isolation forte au niveau SQL (schema séparé = `\dt` ne voit que son tenant)
- Restore par tenant trivial (`pg_dump -n terroir_t_<slug>`)
- Audit + compliance simplifiés
- Migration différentielle possible (un tenant à risque retardé)

### Négatives
- N migrations à orchestrer (CI nécessaire, scripts en place)
- Pool de connexions plus complexe (pgbouncer en mode `transaction` + search_path par session)
- Backup volumes scaling avec N tenants

### Mitigations
- Outil de provisioning automatisé (terroir-admin)
- Monitor migration drift (alarme si tenant > 7j en retard de migration)
- Backup différentiel par tenant + retention adaptée

## Sécurité

- `SET search_path` côté serveur uniquement (jamais accepté du client)
- pgbouncer rejette les requêtes manipulant search_path en client
- Audit : logs de toute requête cross-schema → revue mensuelle RSSI
- Tests de pénétration tenant : tentative d'accès cross-tenant régulière dans suite Playwright
- RLS activé sur tables transverses critiques (defense in depth)

## Coût

- PG : un seul cluster (HA réplica + standby), partitionnement sain
- Estimé : 30-50% économie vs DB-per-tenant à volume cible

## Métriques de succès

- 0 leak cross-tenant détecté (test pen mensuel)
- Provisioning tenant ≤ 5 min P95
- Migration multi-tenant ≤ 30 min total (50 tenants)
- p95 query terroir-core ≤ 100 ms

## Révision

À reconfirmer après 20 tenants en production. Si scaling PG souffre → partitionnement par tenant_id ou citus / yugabytedb.
