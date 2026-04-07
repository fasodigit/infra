/**
 * FASO DIGITALISATION - Poulets Platform
 * Real simulation data for E2E tests with Burkinabe context.
 */

export const eleveurs = [
  {
    name: 'Ouedraogo Amadou',
    email: 'amadou.ouedraogo@test.bf',
    password: 'FasoP0ulet$2026Xk9m',
    phone: '+22670112233',
    role: 'eleveur' as const,
    location: 'Ouagadougou, Secteur 30',
    races: ['local', 'brahma'],
    capacity: 500,
  },
  {
    name: 'Compaore Fatimata',
    email: 'fatimata.compaore@test.bf',
    password: 'FasoP0ulet$2026Xk9m',
    phone: '+22676445566',
    role: 'eleveur' as const,
    location: 'Bobo-Dioulasso, Secteur 8',
    races: ['pintade', 'poulet_chair'],
    capacity: 300,
    groupement: 'Cooperative Avicole du Houet',
  },
];

export const clients = [
  {
    name: 'Restaurant Le Sahel',
    email: 'contact@lesahel.bf',
    password: 'FasoP0ulet$2026Xk9m',
    phone: '+22625334455',
    role: 'client' as const,
    type: 'restaurant' as const,
    location: 'Ouagadougou, Zone du Bois',
  },
  {
    name: 'Traiteur Wendkuni',
    email: 'wendkuni@test.bf',
    password: 'FasoP0ulet$2026Xk9m',
    phone: '+22670998877',
    role: 'client' as const,
    type: 'evenement' as const,
    location: 'Koudougou',
  },
];

export const annonces = [
  {
    race: 'Poulet bicyclette',
    quantity: 100,
    currentWeight: 1.5,
    estimatedWeight: 2.2,
    targetDate: '2026-05-15',
    pricePerKg: 3500,
    pricePerUnit: 7700,
    location: 'Ouagadougou, Secteur 30',
    description: 'Poulets locaux eleves en plein air, alimentation bio',
    halalCertified: true,
    ficheSanitaireId: 'FS-2026-001',
  },
  {
    race: 'Poulet fermier',
    quantity: 50,
    currentWeight: 3.0,
    estimatedWeight: 4.5,
    targetDate: '2026-06-01',
    pricePerKg: 4000,
    pricePerUnit: 18000,
    location: 'Ouagadougou, Secteur 30',
    description: 'Brahma de grande taille, ideal pour evenements',
    halalCertified: true,
    ficheSanitaireId: 'FS-2026-002',
  },
];

export const besoins = [
  {
    races: ['Poulet bicyclette'],
    quantity: 30,
    minWeight: 2.0,
    deliveryDate: '2026-05-20',
    maxBudgetPerKg: 4000,
    frequency: 'HEBDOMADAIRE' as const,
    halalRequired: true,
    vetRequired: true,
    notes: 'Livraison chaque vendredi matin avant 8h',
    location: 'Ouagadougou, Zone du Bois',
  },
  {
    races: ['Poulet fermier', 'Poulet de chair'],
    quantity: 100,
    minWeight: 3.0,
    deliveryDate: '2026-06-15',
    maxBudgetPerKg: 5000,
    frequency: 'PONCTUEL' as const,
    halalRequired: true,
    notes: 'Mariage - besoin de gros poulets',
    location: 'Koudougou',
  },
];

export const vaccinations = [
  {
    vaccin: 'Newcastle (HB1)',
    date: '2026-04-01',
    vet: 'Dr. Sawadogo',
    batchNumber: 'NC-2026-0412',
  },
  {
    vaccin: 'Gumboro',
    date: '2026-04-08',
    vet: 'Dr. Sawadogo',
    batchNumber: 'GB-2026-0815',
  },
];

export const contratRecurrent = {
  race: 'Poulet bicyclette',
  quantity: 30,
  minWeight: 2.0,
  pricePerKg: 3500,
  frequency: 'HEBDOMADAIRE',
  dayPreference: 'vendredi',
  duration: 6, // months
  advancePayment: 10, // percent
  penaltyLate: 5, // percent
  halalRequired: true,
};

/**
 * Timestamp suffix for uniqueness on each test run.
 */
export function uniqueSuffix(): string {
  return Date.now().toString(36);
}

/**
 * Generate a unique email from a base email for test isolation.
 */
export function uniqueEmail(base: string): string {
  const [local, domain] = base.split('@');
  return `${local}+${uniqueSuffix()}@${domain}`;
}
