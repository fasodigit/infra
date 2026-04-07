-- V1__init.sql
-- Initial schema for auth-ms

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ============================================================
-- ROLES
-- ============================================================
CREATE TABLE roles (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- ============================================================
-- PERMISSIONS (Zanzibar-style: namespace#relation@object)
-- ============================================================
CREATE TABLE permissions (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    namespace   VARCHAR(100) NOT NULL,
    object      VARCHAR(255) NOT NULL,
    relation    VARCHAR(100) NOT NULL,
    description TEXT,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    UNIQUE (namespace, object, relation)
);

-- ============================================================
-- ROLE <-> PERMISSION join table
-- ============================================================
CREATE TABLE role_permissions (
    role_id       UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

-- ============================================================
-- USERS
-- ============================================================
CREATE TABLE users (
    id                    UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email                 VARCHAR(255) NOT NULL UNIQUE,
    first_name            VARCHAR(100) NOT NULL,
    last_name             VARCHAR(100) NOT NULL,
    department            VARCHAR(200),
    phone_number          VARCHAR(30),
    active                BOOLEAN      NOT NULL DEFAULT true,
    kratos_identity_id    VARCHAR(255) UNIQUE,
    password_changed_at   TIMESTAMPTZ  NOT NULL DEFAULT now(),
    password_expires_at   TIMESTAMPTZ  NOT NULL DEFAULT (now() + INTERVAL '90 days'),
    locked_until          TIMESTAMPTZ,
    failed_login_attempts INTEGER      NOT NULL DEFAULT 0,
    suspended             BOOLEAN      NOT NULL DEFAULT false,
    created_at            TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- ============================================================
-- USER <-> ROLE join table
-- ============================================================
CREATE TABLE user_roles (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id)
);

-- ============================================================
-- JWT SIGNING KEYS (persisted for rotation tracking)
-- ============================================================
CREATE TABLE jwt_signing_keys (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    kid             VARCHAR(64)  NOT NULL UNIQUE,
    algorithm       VARCHAR(10)  NOT NULL DEFAULT 'ES384',
    public_key_pem  TEXT         NOT NULL,
    private_key_pem TEXT         NOT NULL,
    active          BOOLEAN      NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ  NOT NULL,
    revoked_at      TIMESTAMPTZ
);

CREATE INDEX idx_jwt_signing_keys_active ON jwt_signing_keys(active) WHERE active = true;

-- ============================================================
-- AUDIT LOG
-- ============================================================
CREATE TABLE audit_log (
    id          BIGSERIAL PRIMARY KEY,
    actor_id    UUID,
    action      VARCHAR(100) NOT NULL,
    target_type VARCHAR(100),
    target_id   VARCHAR(255),
    details     JSONB,
    ip_address  VARCHAR(45),
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_log_actor ON audit_log(actor_id);
CREATE INDEX idx_audit_log_created ON audit_log(created_at);

-- ============================================================
-- SEED: default roles
-- ============================================================
INSERT INTO roles (name, description) VALUES
    ('SUPER_ADMIN', 'Full system administrator with all privileges'),
    ('ADMIN', 'Administrative user with user management capabilities'),
    ('OPERATOR', 'Operational staff with limited management access'),
    ('VIEWER', 'Read-only access to dashboards and reports');

-- ============================================================
-- SEED: default permissions
-- ============================================================
INSERT INTO permissions (namespace, object, relation, description) VALUES
    ('auth', 'users', 'create', 'Create new users'),
    ('auth', 'users', 'read', 'View user details'),
    ('auth', 'users', 'update', 'Update user information'),
    ('auth', 'users', 'delete', 'Delete users'),
    ('auth', 'roles', 'manage', 'Manage roles and permissions'),
    ('auth', 'jwt', 'rotate', 'Rotate JWT signing keys'),
    ('auth', 'tokens', 'blacklist', 'Blacklist JWT tokens'),
    ('auth', 'accounts', 'unlock', 'Unlock brute-force-locked accounts');
