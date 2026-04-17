-- =============================================================================
-- FASO DIGITALISATION — outbox-schema.sql
-- Version      : v3.1 (souverain)
-- Cible        : YugabyteDB 2024.2+ (protocole PostgreSQL compatible)
-- Rôle         : Table outbox transactionnelle, partitionnement temporel jour,
--                rôles dédiés (writer, reader relay, admin).
-- Référence    : SPEC-OUTBOX-RELAY-v3.1.md
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 0. Schéma dédié (isolation par module métier : état-civil, hospital, etc.)
--    Usage   : CREATE SCHEMA etat_civil; SET search_path TO etat_civil;
--    Ici on expose la définition générique ; chaque module la réplique.
-- -----------------------------------------------------------------------------

CREATE SCHEMA IF NOT EXISTS outbox_mod;
SET search_path TO outbox_mod, public;

-- -----------------------------------------------------------------------------
-- 1. Extension requises
-- -----------------------------------------------------------------------------

CREATE EXTENSION IF NOT EXISTS "pgcrypto";   -- gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";  -- fallback uuid_generate_v4()

-- -----------------------------------------------------------------------------
-- 2. Nombre de shards de partitionnement horizontal (workers logiques)
--    N = 6 (3 workers × 2 instances HA). Valeur gelée : changer impose rebuild.
-- -----------------------------------------------------------------------------

-- Constante encodée via COMMENT pour audit ; la valeur doit correspondre
-- à la configuration du service outbox-relay.
COMMENT ON SCHEMA outbox_mod IS 'FASO Outbox module — N_SHARDS=6 (v3.1)';

-- -----------------------------------------------------------------------------
-- 3. Table outbox partitionnée par jour (RANGE sur created_at)
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS outbox (
    id                  UUID        NOT NULL DEFAULT gen_random_uuid(),
    aggregate_id        UUID        NOT NULL,
    aggregate_type      VARCHAR(64) NOT NULL,
    event_type          VARCHAR(128) NOT NULL,
    payload             BYTEA       NOT NULL,
    status              VARCHAR(16) NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING','SENT','DEAD_LETTER')),
    idempotency_key     UUID        NOT NULL,
    partition_key       VARCHAR(128) NOT NULL,
    partition_shard     SMALLINT    NOT NULL
        CHECK (partition_shard BETWEEN 0 AND 5),
    retry_count         SMALLINT    NOT NULL DEFAULT 0
        CHECK (retry_count >= 0),
    error_reason        TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ,
    -- PK composite : id + created_at (contrainte de partitionnement natif)
    PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

-- -----------------------------------------------------------------------------
-- 4. Commentaires documentaires sur chaque colonne
-- -----------------------------------------------------------------------------

COMMENT ON TABLE  outbox                  IS 'Transactional outbox pattern — vérité pré-publication (FASO v3.1)';
COMMENT ON COLUMN outbox.id               IS 'Identifiant UUID unique de l''événement (PK logique)';
COMMENT ON COLUMN outbox.aggregate_id     IS 'Identifiant de l''agrégat métier (dossier, patient, ticket...) — clé d''ordering';
COMMENT ON COLUMN outbox.aggregate_type   IS 'Type d''agrégat (ex: etat_civil.dossier, hospital.patient)';
COMMENT ON COLUMN outbox.event_type       IS 'Nom de l''événement versionné (ex: etat_civil.dossier.created.v1)';
COMMENT ON COLUMN outbox.payload          IS 'Payload Avro binaire (schéma enregistré dans FASO Schema Registry)';
COMMENT ON COLUMN outbox.status           IS 'État du cycle de vie : PENDING → SENT | DEAD_LETTER';
COMMENT ON COLUMN outbox.idempotency_key  IS 'UUID d''idempotence, propagé en header Kafka/KAYA pour dédup consumer-side';
COMMENT ON COLUMN outbox.partition_key    IS 'Clé de partition Kafka/KAYA (typiquement aggregate_id stringifié)';
COMMENT ON COLUMN outbox.partition_shard  IS 'Shard worker : murmur3_128(aggregate_id) mod 6 — figé à l''INSERT';
COMMENT ON COLUMN outbox.retry_count      IS 'Nombre de tentatives de publication échouées';
COMMENT ON COLUMN outbox.error_reason     IS 'Dernière cause d''échec (512 chars recommandés, TEXT autorise plus)';
COMMENT ON COLUMN outbox.created_at       IS 'Horodatage d''écriture (fait partie de la PK composite pour partitionnement)';
COMMENT ON COLUMN outbox.updated_at       IS 'Horodatage du dernier changement de statut';

-- -----------------------------------------------------------------------------
-- 5. Partitions filles : 7 jours roulants pré-créés
--    En prod, un cron `outbox-gc` les crée J-1 à 02:00 UTC et drop J-8 SENT.
--    Ici on expose la convention ; remplacer les dates par CURRENT_DATE + N.
-- -----------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS outbox_2026_04_16 PARTITION OF outbox
    FOR VALUES FROM ('2026-04-16 00:00:00+00') TO ('2026-04-17 00:00:00+00');

CREATE TABLE IF NOT EXISTS outbox_2026_04_17 PARTITION OF outbox
    FOR VALUES FROM ('2026-04-17 00:00:00+00') TO ('2026-04-18 00:00:00+00');

CREATE TABLE IF NOT EXISTS outbox_2026_04_18 PARTITION OF outbox
    FOR VALUES FROM ('2026-04-18 00:00:00+00') TO ('2026-04-19 00:00:00+00');

CREATE TABLE IF NOT EXISTS outbox_2026_04_19 PARTITION OF outbox
    FOR VALUES FROM ('2026-04-19 00:00:00+00') TO ('2026-04-20 00:00:00+00');

CREATE TABLE IF NOT EXISTS outbox_2026_04_20 PARTITION OF outbox
    FOR VALUES FROM ('2026-04-20 00:00:00+00') TO ('2026-04-21 00:00:00+00');

CREATE TABLE IF NOT EXISTS outbox_2026_04_21 PARTITION OF outbox
    FOR VALUES FROM ('2026-04-21 00:00:00+00') TO ('2026-04-22 00:00:00+00');

CREATE TABLE IF NOT EXISTS outbox_2026_04_22 PARTITION OF outbox
    FOR VALUES FROM ('2026-04-22 00:00:00+00') TO ('2026-04-23 00:00:00+00');

-- Partition default : absorbe les écritures hors plage (devrait rester vide).
CREATE TABLE IF NOT EXISTS outbox_default PARTITION OF outbox DEFAULT;

-- -----------------------------------------------------------------------------
-- 6. Index
-- -----------------------------------------------------------------------------

-- 6.1. Index partiel de la boucle relay (cœur de perf) — doit exister sur
--      chaque partition. On le pose sur la table mère (YugabyteDB le propage).
CREATE INDEX IF NOT EXISTS idx_outbox_pending
    ON outbox (partition_shard, created_at)
    WHERE status = 'PENDING';

COMMENT ON INDEX idx_outbox_pending IS
    'Index partiel — boucle worker outbox-relay. Réduit drastiquement la taille vs index plein.';

-- 6.2. Index aggregate pour debug / replay per-aggregate_id
CREATE INDEX IF NOT EXISTS idx_outbox_aggregate
    ON outbox (aggregate_id, created_at);

COMMENT ON INDEX idx_outbox_aggregate IS
    'Replay par agrégat (ex: rejouer toute l''historique d''un dossier)';

-- 6.3. Index unique sur idempotency_key (sécurité anti-doublon writer)
CREATE UNIQUE INDEX IF NOT EXISTS idx_outbox_idempotency
    ON outbox (idempotency_key);

COMMENT ON INDEX idx_outbox_idempotency IS
    'Contrainte d''unicité sur la clé d''idempotence (1 événement = 1 clé)';

-- -----------------------------------------------------------------------------
-- 7. Rôles
-- -----------------------------------------------------------------------------

-- outbox_writer : utilisé par ingestion-rs. INSERT seul.
CREATE ROLE outbox_writer LOGIN
    PASSWORD NULL                          -- auth SPIRE SVID + mTLS
    NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION;
GRANT USAGE ON SCHEMA outbox_mod TO outbox_writer;
GRANT INSERT ON outbox TO outbox_writer;
-- Pas de SELECT, pas d'UPDATE : l'écriture est fire-and-forget.

-- outbox_reader_relay : utilisé par outbox-relay. SELECT + UPDATE status/retry.
CREATE ROLE outbox_reader_relay LOGIN
    PASSWORD NULL
    NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION;
GRANT USAGE ON SCHEMA outbox_mod TO outbox_reader_relay;
GRANT SELECT ON outbox TO outbox_reader_relay;
GRANT UPDATE (status, retry_count, error_reason, updated_at) ON outbox TO outbox_reader_relay;
-- Pas de DELETE, pas d'INSERT, pas d'UPDATE sur payload/id/aggregate_id.

-- outbox_admin : DDL, rotation partitions, backups.
CREATE ROLE outbox_admin LOGIN
    PASSWORD NULL
    NOINHERIT CREATEDB NOCREATEROLE NOREPLICATION;
GRANT ALL PRIVILEGES ON SCHEMA outbox_mod TO outbox_admin;
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA outbox_mod TO outbox_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA outbox_mod
    GRANT ALL PRIVILEGES ON TABLES TO outbox_admin;

-- -----------------------------------------------------------------------------
-- 8. Fonctions utilitaires
-- -----------------------------------------------------------------------------

-- Fonction de calcul du shard côté serveur (fallback si le writer ne le fait pas).
-- Usage : INSERT ... partition_shard = outbox_compute_shard(aggregate_id);
CREATE OR REPLACE FUNCTION outbox_compute_shard(p_aggregate_id UUID)
RETURNS SMALLINT
LANGUAGE SQL
IMMUTABLE
AS $$
    -- YugabyteDB/PG : hashtext() suffit pour répartition uniforme.
    -- En production, on préfère murmur3_128 côté application (Rust mur3).
    SELECT (abs(hashtext(p_aggregate_id::text)) % 6)::SMALLINT;
$$;

COMMENT ON FUNCTION outbox_compute_shard(UUID) IS
    'Calcule le shard [0..5] pour un aggregate_id. Doit rester cohérent avec le hash Rust.';

-- -----------------------------------------------------------------------------
-- 9. Vue opérationnelle (lecture on-call runbook)
-- -----------------------------------------------------------------------------

CREATE OR REPLACE VIEW outbox_status_summary AS
SELECT
    status,
    partition_shard,
    COUNT(*)                        AS nb,
    MIN(created_at)                 AS oldest,
    MAX(created_at)                 AS newest,
    AVG(retry_count)::NUMERIC(5,2)  AS avg_retries
FROM outbox
GROUP BY status, partition_shard
ORDER BY status, partition_shard;

COMMENT ON VIEW outbox_status_summary IS
    'Tableau de bord on-call : répartition par statut × shard';

GRANT SELECT ON outbox_status_summary TO outbox_reader_relay, outbox_admin;

-- =============================================================================
-- Fin du schéma. Vérification rapide :
--   SELECT * FROM outbox_status_summary;
--   \d+ outbox
-- =============================================================================
