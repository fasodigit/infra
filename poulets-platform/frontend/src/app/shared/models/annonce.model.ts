import { Race } from './poulet.model';

export interface AnnonceEleveur {
  id: string;
  titre: string;
  description: string;
  race: Race;
  quantite: number;
  prixUnitaire: number;
  poidsMin?: number;
  poidsMax?: number;
  disponibleDe: string;
  disponibleA?: string;
  localisation: string;
  photos?: string[];
  eleveurId: string;
  eleveurNom?: string;
  statut: 'ACTIVE' | 'EXPIREE' | 'VENDUE' | 'SUSPENDUE';
  createdAt: string;
  updatedAt?: string;
}

export interface BesoinClient {
  id: string;
  titre: string;
  description?: string;
  race?: Race;
  quantite: number;
  prixMaxUnitaire?: number;
  dateSouhaitee: string;
  localisation: string;
  clientId: string;
  clientNom?: string;
  statut: 'ACTIF' | 'SATISFAIT' | 'EXPIRE' | 'ANNULE';
  createdAt: string;
}

export interface MatchResult {
  id: string;
  annonce: AnnonceEleveur;
  besoin: BesoinClient;
  score: number; // matching score 0-100
  distance?: number; // in km
  prixDifference?: number;
  createdAt: string;
}
