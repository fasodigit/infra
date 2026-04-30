// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// 500-record dataset generator for FASO E2E. Distribution & cascading
// artifacts are designed to exercise EVERY platform feature × EVERY role
// at scale, not just happy-path with 25 actors.

import { faker, fakerFR } from '@faker-js/faker';
import type { Actor, ActorRole } from './actors';

/**
 * Distribution choisie pour révéler les bugs de scale :
 *  - 200 eleveurs × 200 clients pour saturer le matching
 *  - 5 admins (suffisant)
 *  - rôles métier rares (vétérinaire, vaccins) → cas edge
 *
 * = 500 actors × ~5 cascading artifacts/actor = ~2500 entities en DB.
 */
export const SEED_DISTRIBUTION = {
  eleveur:      200, // 40 % — coeur de l'offre
  client:       200, // 40 % — coeur de la demande
  pharmacie:     30, //  6 %
  veterinaire:   20, //  4 %
  aliments:      20, //  4 %
  transporteur:  20, //  4 %
  vaccins:        5, //  1 %
  admin:          5, //  1 %
} as const;

const SEED = 42;
const BURKINA_REGIONS = [
  'Centre', 'Hauts-Bassins', 'Boucle du Mouhoun', 'Cascades',
  'Centre-Est', 'Centre-Nord', 'Centre-Ouest', 'Centre-Sud',
  'Est', 'Nord', 'Plateau-Central', 'Sahel', 'Sud-Ouest',
];
const BURKINA_CITIES = [
  'Ouagadougou', 'Bobo-Dioulasso', 'Koudougou', 'Banfora',
  'Ouahigouya', 'Kaya', 'Tenkodogo', 'Fada N\'Gourma',
  'Dedougou', 'Dori', 'Ziniare', 'Pouytenga', 'Manga', 'Gaoua',
];

function bfPhone(): string {
  const prefixes = ['70','71','72','73','74','75','76','77','78','79'];
  const prefix = prefixes[faker.number.int({ min: 0, max: prefixes.length - 1 })] ?? '70';
  return `+226${prefix}${faker.string.numeric(6)}`;
}

function makeActor(role: ActorRole, index: number): Actor {
  const firstName = fakerFR.person.firstName();
  const lastName  = fakerFR.person.lastName();
  const slug      = faker.helpers.slugify(`${firstName}.${lastName}`.toLowerCase());
  const a: Actor = {
    id:        `${role}-${index}`,
    role,
    firstName,
    lastName,
    email:     `${slug}.${role}.${index}@faso-e2e.test`,
    phone:     bfPhone(),
    password:  'FasoTest2026!',
    city:      BURKINA_CITIES[faker.number.int({ min: 0, max: BURKINA_CITIES.length - 1 })] ?? 'Ouagadougou',
    region:    BURKINA_REGIONS[faker.number.int({ min: 0, max: BURKINA_REGIONS.length - 1 })] ?? 'Centre',
  };
  if (role === 'eleveur')      a.siret   = faker.string.numeric(14);
  if (role === 'pharmacie' || role === 'vaccins' || role === 'veterinaire') {
    a.amm     = `AMM-BF-${faker.string.numeric(5)}-${faker.string.alpha({ length: 2, casing: 'upper' })}`;
    a.licence = `LIC-BF-${faker.string.numeric(6)}`;
    a.company = fakerFR.company.name();
  }
  if (role === 'aliments') {
    a.siret   = faker.string.numeric(14);
    a.company = fakerFR.company.name();
  }
  return a;
}

/**
 * Génère 500 acteurs déterministes selon SEED_DISTRIBUTION.
 * Idempotent : la même seed rend toujours les mêmes 500 actors → tests
 * reproductibles et debugables.
 */
export function gen500Actors(): Actor[] {
  fakerFR.seed(SEED);
  faker.seed(SEED);
  const out: Actor[] = [];
  let idx = 0;
  for (const [role, count] of Object.entries(SEED_DISTRIBUTION)) {
    for (let i = 0; i < count; i++) {
      out.push(makeActor(role as ActorRole, idx++));
    }
  }
  return out;
}

/** Sample helper for matrix tests : pick a random actor of a given role. */
export function pickRandomActor(actors: Actor[], role: ActorRole): Actor {
  const candidates = actors.filter(a => a.role === role);
  if (candidates.length === 0) throw new Error(`No actor with role ${role} in dataset`);
  return candidates[Math.floor(Math.random() * candidates.length)]!;
}

// ── Cascading artifacts ────────────────────────────────────────────────

export interface OfferRecord {
  id:            string;
  eleveurId:     string;
  category:      string;
  quantity:      number;
  pricePerKg:    number;
  description:   string;
  halalCertified: boolean;
}

export interface DemandRecord {
  id:           string;
  clientId:     string;
  category:     string;
  quantity:     number;
  maxBudgetXof: number;
  location:     string;
}

export interface OrderRecord {
  id:        string;
  offerId:   string;
  demandId:  string;
  clientId:  string;
  eleveurId: string;
  status:    'EN_ATTENTE' | 'CONFIRMEE' | 'LIVREE' | 'ANNULEE';
}

const POULET_RACES = ['Poulet local', 'Wassachie', 'Kabir', 'Hubbard', 'Ross-308'];

/** ~3 offers per eleveur → ~600 offers across 200 eleveurs. */
export function genOffers(actors: Actor[]): OfferRecord[] {
  fakerFR.seed(SEED + 1);
  faker.seed(SEED + 1);
  const eleveurs = actors.filter(a => a.role === 'eleveur');
  const out: OfferRecord[] = [];
  for (const e of eleveurs) {
    const n = faker.number.int({ min: 0, max: 5 });
    for (let i = 0; i < n; i++) {
      out.push({
        id:        `offer-${e.id}-${i}`,
        eleveurId: e.id,
        category:  POULET_RACES[faker.number.int({ min: 0, max: POULET_RACES.length - 1 })] ?? 'Poulet local',
        quantity:  faker.number.int({ min: 10, max: 500 }),
        pricePerKg: faker.number.int({ min: 1500, max: 4500 }),
        description: fakerFR.lorem.sentence(),
        halalCertified: faker.datatype.boolean(0.7),
      });
    }
  }
  return out;
}

/** ~1.5 demands per client → ~300 demands across 200 clients. */
export function genDemands(actors: Actor[]): DemandRecord[] {
  fakerFR.seed(SEED + 2);
  faker.seed(SEED + 2);
  const clients = actors.filter(a => a.role === 'client');
  const out: DemandRecord[] = [];
  for (const c of clients) {
    const n = faker.number.int({ min: 0, max: 3 });
    for (let i = 0; i < n; i++) {
      out.push({
        id:        `demand-${c.id}-${i}`,
        clientId:  c.id,
        category:  POULET_RACES[faker.number.int({ min: 0, max: POULET_RACES.length - 1 })] ?? 'Poulet local',
        quantity:  faker.number.int({ min: 5, max: 200 }),
        maxBudgetXof: faker.number.int({ min: 5000, max: 500_000 }),
        location:  c.city,
      });
    }
  }
  return out;
}

/**
 * Naive matching: for each demand, find a compatible offer and create an
 * order. ~1 order per demand → ~300 orders.
 */
export function genOrders(offers: OfferRecord[], demands: DemandRecord[], actors: Actor[]): OrderRecord[] {
  fakerFR.seed(SEED + 3);
  faker.seed(SEED + 3);
  const out: OrderRecord[] = [];
  for (const d of demands) {
    const compatible = offers.filter(o =>
      o.category === d.category &&
      o.quantity >= Math.ceil(d.quantity * 0.8),
    );
    if (compatible.length === 0) continue;
    const o = compatible[faker.number.int({ min: 0, max: compatible.length - 1 })]!;
    const eleveur = actors.find(a => a.id === o.eleveurId)!;
    out.push({
      id:        `order-${d.id}`,
      offerId:   o.id,
      demandId:  d.id,
      clientId:  d.clientId,
      eleveurId: eleveur.id,
      status:    faker.helpers.arrayElement(['EN_ATTENTE', 'CONFIRMEE', 'LIVREE', 'ANNULEE']),
    });
  }
  return out;
}

/** Counters for assertions in seed-data.ts. */
export interface SeedSnapshot {
  createdAt: number;
  counts: {
    actors:     number;
    offers:     number;
    demands:    number;
    orders:     number;
  };
  byRole: Record<ActorRole, number>;
}

export function snapshot(actors: Actor[], offers: OfferRecord[], demands: DemandRecord[], orders: OrderRecord[]): SeedSnapshot {
  const byRole = actors.reduce((acc, a) => {
    acc[a.role] = (acc[a.role] ?? 0) + 1;
    return acc;
  }, {} as Record<ActorRole, number>);
  return {
    createdAt: Date.now(),
    counts: {
      actors: actors.length,
      offers: offers.length,
      demands: demands.length,
      orders: orders.length,
    },
    byRole,
  };
}
