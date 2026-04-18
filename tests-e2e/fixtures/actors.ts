import { faker, fakerFR } from '@faker-js/faker';

export type ActorRole =
  | 'eleveur'
  | 'pharmacie'
  | 'aliments'
  | 'vaccins'
  | 'client'
  | 'admin'
  | 'veterinaire'
  | 'transporteur';

export interface Actor {
  id: string;
  role: ActorRole;
  firstName: string;
  lastName: string;
  email: string;
  phone: string;
  password: string;
  city: string;
  region: string;
  siret?: string;
  amm?: string;
  licence?: string;
  company?: string;
}

const SEED = 42;

fakerFR.seed(SEED);
faker.seed(SEED);

const BURKINA_REGIONS = [
  'Centre',
  'Hauts-Bassins',
  'Boucle du Mouhoun',
  'Cascades',
  'Centre-Est',
  'Centre-Nord',
  'Centre-Ouest',
  'Centre-Sud',
  'Est',
  'Nord',
  'Plateau-Central',
  'Sahel',
  'Sud-Ouest',
];

const BURKINA_CITIES = [
  'Ouagadougou',
  'Bobo-Dioulasso',
  'Koudougou',
  'Banfora',
  'Ouahigouya',
  'Kaya',
  'Tenkodogo',
  'Fada N\'Gourma',
  'Dedougou',
  'Dori',
  'Ziniare',
  'Pouytenga',
  'Manga',
  'Gaoua',
];

function bfPhone(): string {
  const prefixes = ['70', '71', '72', '73', '74', '75', '76', '77', '78', '79'];
  const prefix = prefixes[faker.number.int({ min: 0, max: prefixes.length - 1 })] ?? '70';
  const rest = faker.string.numeric(6);
  return `+226${prefix}${rest}`;
}

function bfCity(): string {
  return BURKINA_CITIES[faker.number.int({ min: 0, max: BURKINA_CITIES.length - 1 })] ?? 'Ouagadougou';
}

function bfRegion(): string {
  return BURKINA_REGIONS[faker.number.int({ min: 0, max: BURKINA_REGIONS.length - 1 })] ?? 'Centre';
}

function siret(): string {
  return faker.string.numeric(14);
}

function amm(): string {
  return `AMM-BF-${faker.string.numeric(5)}-${faker.string.alpha({ length: 2, casing: 'upper' })}`;
}

function licence(): string {
  return `LIC-BF-${faker.string.numeric(6)}`;
}

function makeActor(role: ActorRole, index: number): Actor {
  const firstName = fakerFR.person.firstName();
  const lastName = fakerFR.person.lastName();
  const emailSlug = faker.helpers.slugify(`${firstName}.${lastName}`.toLowerCase());
  const email = `${emailSlug}.${role}.${index}@faso-e2e.test`;
  const base: Actor = {
    id: `${role}-${index}`,
    role,
    firstName,
    lastName,
    email,
    phone: bfPhone(),
    password: 'FasoTest2026!',
    city: bfCity(),
    region: bfRegion(),
  };
  if (role === 'eleveur') base.siret = siret();
  if (role === 'pharmacie' || role === 'vaccins' || role === 'veterinaire') {
    base.amm = amm();
    base.licence = licence();
    base.company = fakerFR.company.name();
  }
  if (role === 'aliments') {
    base.siret = siret();
    base.company = fakerFR.company.name();
  }
  return base;
}

export const actors25: Actor[] = [
  ...Array.from({ length: 5 }, (_, i) => makeActor('eleveur', i + 1)),
  ...Array.from({ length: 5 }, (_, i) => makeActor('pharmacie', i + 1)),
  ...Array.from({ length: 5 }, (_, i) => makeActor('aliments', i + 1)),
  ...Array.from({ length: 5 }, (_, i) => makeActor('vaccins', i + 1)),
  ...Array.from({ length: 5 }, (_, i) => makeActor('client', i + 1)),
];

export function actorsByRole(role: ActorRole): Actor[] {
  return actors25.filter((a) => a.role === role);
}

export function gen1000Clients(): Actor[] {
  const localFaker = fakerFR;
  localFaker.seed(SEED + 1);
  faker.seed(SEED + 1);
  return Array.from({ length: 1000 }, (_, i) => makeActor('client', i + 1000));
}
