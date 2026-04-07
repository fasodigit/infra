export type Role = 'eleveur' | 'client' | 'producteur_aliment' | 'admin';

export type ClientType = 'restaurant' | 'menage' | 'revendeur' | 'evenement';

export interface Groupement {
  id: string;
  nom: string;
  description?: string;
  localisation?: string;
  membresCount?: number;
  createdAt?: string;
}

export interface User {
  id: string;
  email: string;
  nom: string;
  prenom?: string;
  phone?: string;
  role: Role;
  verified: boolean;
  avatar?: string;
  localisation?: string;
  groupement?: Groupement;
  createdAt?: string;
  updatedAt?: string;
}

export interface EleveurProfile extends User {
  role: 'eleveur';
  racesElevees?: string[];
  capacite?: number;
  note?: number;
  totalVentes?: number;
}

export interface ClientProfile extends User {
  role: 'client';
  clientType?: ClientType;
}

export interface ProducteurProfile extends User {
  role: 'producteur_aliment';
  produits?: string[];
  zoneDistribution?: string;
}

export interface UserSession {
  id: string;
  email: string;
  nom: string;
  prenom?: string;
  phone?: string;
  role: Role;
  verified: boolean;
  avatar?: string;
  groupement?: Groupement;
}

export interface LoginRequest {
  email: string;
  password: string;
}

export interface RegisterRequest {
  email: string;
  password: string;
  nom: string;
  phone?: string;
  role: Role;
  // Eleveur specifics
  localisation?: string;
  racesElevees?: string[];
  capacite?: number;
  // Client specifics
  clientType?: ClientType;
  // Producteur specifics
  produits?: string[];
  zoneDistribution?: string;
  // Groupement
  groupementId?: string;
  groupementNom?: string;
}
