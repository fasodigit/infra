-- V2__encrypt_jwt_keys.sql
-- Marks existing plaintext private keys for re-encryption at application startup.
--
-- The actual AES-256-GCM encryption is performed by EncryptedStringConverter
-- the first time each row is loaded and re-saved by the key-rotation job.
-- This migration adds a helper column so the startup migration hook can identify
-- rows that have not yet been encrypted (plaintext PEM starts with "-----BEGIN").
--
-- NOTE: Run the KeyRotationMigrationService bean on first startup with
--       JWT_KEY_ENCRYPTION_KEY set to re-encrypt all existing plaintext keys.

ALTER TABLE jwt_signing_keys
    ADD COLUMN IF NOT EXISTS key_encrypted BOOLEAN NOT NULL DEFAULT false;

-- Mark all existing rows as needing encryption
UPDATE jwt_signing_keys SET key_encrypted = false WHERE key_encrypted IS DISTINCT FROM true;
