/**
 * Marketplace domain models for annonces, besoins, and matching.
 */

export interface Annonce {
  id: string;
  eleveurId: string;
  eleveur: AnnonceEleveur;
  race: string;
  quantity: number;
  currentWeight: number;
  estimatedWeight: number;
  targetDate: string;
  pricePerKg: number;
  pricePerUnit: number;
  location: string;
  latitude?: number;
  longitude?: number;
  description: string;
  photos: string[];
  availabilityStart: string;
  availabilityEnd: string;
  veterinaryStatus: VeterinaryStatus;
  ficheSanitaireId?: string;
  halalCertified: boolean;
  isGroupement: boolean;
  groupementId?: string;
  status: AnnonceStatus;
  createdAt: string;
  updatedAt: string;
}

export interface AnnonceEleveur {
  id: string;
  nom: string;
  prenom?: string;
  localisation: string;
  note: number;
  responseTime?: string;
  ponctualite?: number;
  totalVentes?: number;
  telephone?: string;
}

export type AnnonceStatus = 'ACTIVE' | 'EXPIREE' | 'VENDUE' | 'SUSPENDUE';

export type VeterinaryStatus = 'VERIFIED' | 'PENDING' | 'NOT_PROVIDED';

export interface Besoin {
  id: string;
  clientId: string;
  client: BesoinClient;
  races: string[];
  quantity: number;
  minimumWeight: number;
  deliveryDate: string;
  maxBudgetPerKg: number;
  location: string;
  latitude?: number;
  longitude?: number;
  frequency: BesoinFrequency;
  recurringStartDate?: string;
  recurringEndDate?: string;
  dayOfWeekPreference?: number;
  halalRequired: boolean;
  veterinaryCertifiedRequired: boolean;
  specialNotes?: string;
  status: BesoinStatus;
  createdAt: string;
  updatedAt: string;
}

export interface BesoinClient {
  id: string;
  nom: string;
  localisation: string;
}

export type BesoinFrequency = 'PONCTUEL' | 'HEBDOMADAIRE' | 'BI_MENSUEL' | 'MENSUEL';

export type BesoinStatus = 'ACTIVE' | 'SATISFAIT' | 'EXPIRE' | 'ANNULE';

export interface MatchResult {
  id: string;
  annonce?: Annonce;
  besoin?: Besoin;
  matchScore: number;
  raceCompatibility: number;
  weightFeasibility: number;
  dateCompatibility: number;
  proximity: number;
  reputation: number;
}

export interface AnnonceFilter {
  race?: string;
  weightMin?: number;
  weightMax?: number;
  priceMin?: number;
  priceMax?: number;
  location?: string;
  dateFrom?: string;
  dateTo?: string;
  halalOnly?: boolean;
  veterinaryVerified?: boolean;
}

export interface BesoinFilter {
  race?: string;
  quantityMin?: number;
  budgetMin?: number;
  budgetMax?: number;
  location?: string;
  frequency?: BesoinFrequency;
}

export interface CreateAnnonceInput {
  race: string;
  quantity: number;
  currentWeight: number;
  estimatedWeight: number;
  targetDate: string;
  pricePerKg: number;
  pricePerUnit: number;
  location: string;
  latitude?: number;
  longitude?: number;
  description: string;
  photos: string[];
  availabilityStart: string;
  availabilityEnd: string;
  ficheSanitaireId?: string;
  halalCertified: boolean;
  isGroupement: boolean;
  groupementId?: string;
}

export interface CreateBesoinInput {
  races: string[];
  quantity: number;
  minimumWeight: number;
  deliveryDate: string;
  maxBudgetPerKg: number;
  location: string;
  latitude?: number;
  longitude?: number;
  frequency: BesoinFrequency;
  recurringStartDate?: string;
  recurringEndDate?: string;
  dayOfWeekPreference?: number;
  halalRequired: boolean;
  veterinaryCertifiedRequired: boolean;
  specialNotes?: string;
}

/** Available chicken races for dropdowns */
export const CHICKEN_RACES: string[] = [
  'Poulet bicyclette',
  'Poulet de chair',
  'Coq local',
  'Pintade',
  'Dinde',
  'Poule pondeuse',
  'Poulet fermier',
  'Coquelet',
];

/** Days of week for recurring schedules */
export const DAYS_OF_WEEK: { value: number; label: string }[] = [
  { value: 1, label: 'Lundi' },
  { value: 2, label: 'Mardi' },
  { value: 3, label: 'Mercredi' },
  { value: 4, label: 'Jeudi' },
  { value: 5, label: 'Vendredi' },
  { value: 6, label: 'Samedi' },
  { value: 0, label: 'Dimanche' },
];
