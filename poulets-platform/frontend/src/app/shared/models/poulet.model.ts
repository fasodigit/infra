export enum Race {
  LOCAL = 'LOCAL',
  BICYCLETTE = 'BICYCLETTE',
  BRAHMA = 'BRAHMA',
  SUSSEX = 'SUSSEX',
  RHODE_ISLAND = 'RHODE_ISLAND',
  LEGHORN = 'LEGHORN',
  COUCOU = 'COUCOU',
  PINTADE = 'PINTADE',
  DINDE = 'DINDE',
  MIXED = 'MIXED',
}

export enum PouletStatut {
  DISPONIBLE = 'DISPONIBLE',
  RESERVE = 'RESERVE',
  VENDU = 'VENDU',
}

export interface Poulet {
  id: string;
  race: Race;
  age: number; // in weeks
  poids: number; // in kg
  prix: number; // in FCFA
  statut: PouletStatut;
  description?: string;
  photos?: string[];
  alimentation?: string;
  vaccinations?: string[];
  certificats?: string[];
  lotId?: string;
  eleveurId: string;
  eleveurNom?: string;
  eleveurLocalisation?: string;
  eleveurNote?: number;
  createdAt: string;
  updatedAt?: string;
}

export interface Lot {
  id: string;
  nom: string;
  race: Race;
  effectifInitial: number;
  effectifActuel: number;
  dateArrivee: string;
  ageArrivee: number; // in weeks
  poidsArrivee: number; // in kg
  poidsMoyen: number; // current average weight
  tauxMortalite: number;
  indiceConversion?: number;
  statut: 'EN_COURS' | 'TERMINE' | 'VENDU';
  mesures: MesureCroissance[];
  eleveurId: string;
  createdAt: string;
}

export interface MesureCroissance {
  id: string;
  lotId: string;
  date: string;
  poidsMoyen: number;
  effectif: number;
  alimentConsomme?: number;
  observations?: string;
}

export interface Poussin {
  id: string;
  producteur: string;
  producteur_id: string;
  race: string;
  age_jours: number;       // 1, 7, 14, 21 days
  quantity: number;
  price_unit: number;       // FCFA per chick
  vaccinated: boolean;      // Marek, Newcastle
  vaccination_details?: string;
  location: string;
  region: string;
  available_from: string;
  status: 'active' | 'reserve' | 'epuise';
  created_at: string;
}

export interface PouletFilter {
  race?: Race;
  prixMin?: number;
  prixMax?: number;
  poidsMin?: number;
  poidsMax?: number;
  localisation?: string;
  statut?: PouletStatut;
}

export interface Page<T> {
  content: T[];
  totalElements: number;
  totalPages: number;
  currentPage: number;
}
