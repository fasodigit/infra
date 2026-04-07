-- V1__init.sql
-- Initial schema for poulets-api

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ============================================================
-- CATEGORIES
-- ============================================================
CREATE TABLE categories (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        VARCHAR(50) NOT NULL UNIQUE,
    description TEXT
);

INSERT INTO categories (name, description) VALUES
    ('POULET_CHAIR', 'Poulet de chair - elevage pour la viande'),
    ('POULET_PONDEUSE', 'Poule pondeuse - elevage pour les oeufs'),
    ('PINTADE', 'Pintade - volaille locale prisee'),
    ('OEUF', 'Oeufs frais de ferme'),
    ('ALIMENT', 'Aliment pour volaille');

-- ============================================================
-- ELEVEURS (farmers)
-- ============================================================
CREATE TABLE eleveurs (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id     VARCHAR(255) NOT NULL,
    name        VARCHAR(200) NOT NULL,
    phone       VARCHAR(30)  NOT NULL,
    location    VARCHAR(300) NOT NULL,
    description TEXT,
    rating      DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    active      BOOLEAN      NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE INDEX idx_eleveurs_user_id ON eleveurs(user_id);
CREATE INDEX idx_eleveurs_location ON eleveurs(location);

-- ============================================================
-- CLIENTS (buyers)
-- ============================================================
CREATE TABLE clients (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id     VARCHAR(255) NOT NULL,
    name        VARCHAR(200) NOT NULL,
    phone       VARCHAR(30)  NOT NULL,
    address     VARCHAR(500) NOT NULL,
    active      BOOLEAN      NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE INDEX idx_clients_user_id ON clients(user_id);

-- ============================================================
-- POULETS (chickens)
-- ============================================================
CREATE TABLE poulets (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    eleveur_id    UUID         NOT NULL REFERENCES eleveurs(id) ON DELETE CASCADE,
    race          VARCHAR(20)  NOT NULL,
    weight        DOUBLE PRECISION NOT NULL,
    price         DOUBLE PRECISION NOT NULL,
    quantity      INTEGER      NOT NULL DEFAULT 0,
    description   TEXT,
    available     BOOLEAN      NOT NULL DEFAULT true,
    categorie_id  UUID         REFERENCES categories(id),
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE INDEX idx_poulets_eleveur_id ON poulets(eleveur_id);
CREATE INDEX idx_poulets_race ON poulets(race);
CREATE INDEX idx_poulets_available ON poulets(available) WHERE available = true;
CREATE INDEX idx_poulets_categorie_id ON poulets(categorie_id);

-- ============================================================
-- COMMANDES (orders)
-- ============================================================
CREATE TABLE commandes (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    client_id     UUID         NOT NULL REFERENCES clients(id),
    eleveur_id    UUID         NOT NULL REFERENCES eleveurs(id),
    status        VARCHAR(20)  NOT NULL DEFAULT 'PENDING',
    total_amount  DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT now(),
    delivered_at  TIMESTAMPTZ,
    updated_at    TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE INDEX idx_commandes_client_id ON commandes(client_id);
CREATE INDEX idx_commandes_eleveur_id ON commandes(eleveur_id);
CREATE INDEX idx_commandes_status ON commandes(status);

-- ============================================================
-- COMMANDE_ITEMS
-- ============================================================
CREATE TABLE commande_items (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    commande_id   UUID         NOT NULL REFERENCES commandes(id) ON DELETE CASCADE,
    poulet_id     UUID         NOT NULL REFERENCES poulets(id),
    quantity      INTEGER      NOT NULL,
    unit_price    DOUBLE PRECISION NOT NULL,
    created_at    TIMESTAMPTZ  NOT NULL DEFAULT now()
);

CREATE INDEX idx_commande_items_commande_id ON commande_items(commande_id);
