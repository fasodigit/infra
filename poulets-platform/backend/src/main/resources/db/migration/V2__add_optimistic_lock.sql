-- V2__add_optimistic_lock.sql
-- Adds JPA @Version column to poulets for optimistic locking (race condition fix).

ALTER TABLE poulets
    ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 0;
