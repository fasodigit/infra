# notifier-ms — Database Migration Guide

## Flyway Configuration

- Location: `notifier-core/src/main/resources/db/migration/`
- Naming: `V{version}__{description}.sql`
- DB: PostgreSQL 17, schema: `public`, database: `notifier`
- Profile: `spring.flyway.enabled=true` (disabled in `test` profile)

## Migrations

### V1__init.sql
**Tables created:**
- `notification_templates` — Handlebars template storage with JSONB context rules
- `notification_deliveries` — Per-delivery idempotency tracking (delivery_id PK)
- `notification_recipients` — Named recipient groups for rule resolution

**Seed data:**
- 10 default recipient groups (`devops`, `agriculture-metier`, `etat-civil`, etc.)

## Running Migrations Manually

```bash
# Apply pending migrations
mvn flyway:migrate -pl notifier-core \
  -Dflyway.url=jdbc:postgresql://localhost:5432/notifier \
  -Dflyway.user=notifier \
  -Dflyway.password=<password>

# Check migration status
mvn flyway:info -pl notifier-core \
  -Dflyway.url=jdbc:postgresql://localhost:5432/notifier \
  -Dflyway.user=notifier \
  -Dflyway.password=<password>

# Repair checksum mismatches (dev only, never in prod)
mvn flyway:repair -pl notifier-core ...
```

## Adding a New Migration

1. Create `V{N+1}__{description}.sql` in `db/migration/`
2. Never modify an existing migration after deployment
3. Test locally: `mvn flyway:migrate` against a clean DB
4. Add rollback notes as SQL comments in the migration file

## Schema Diagram

```
notification_templates
  id (PK)
  name (UNIQUE)
  subject_template
  body_hbs (TEXT)
  context_rules_json (JSONB)
  created_at, updated_at

notification_deliveries
  delivery_id (PK) ← deterministic: {event_id}_{rule_id}_{recipient_hash}
  recipient
  template_name → FK (soft, name-based)
  status: PENDING | SENT | FAILED | DLQ
  attempts
  last_error (TEXT)
  event_payload (TEXT) ← for DLQ replay
  sent_at, created_at, updated_at

notification_recipients
  id (PK)
  group_name + email (UNIQUE)
  label
  active
  created_at
```

## Sovereign Constraints

- KAYA deduplication (7-day TTL) operates independently of DB — no FK to KAYA keys
- `delivery_id` is generated deterministically: allows DB-side idempotency checks
  even if KAYA restarts and loses dedup keys
