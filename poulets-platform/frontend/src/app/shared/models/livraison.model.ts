export enum ModeLivraison {
  MOTO = 'MOTO',
  VOITURE = 'VOITURE',
  CAMION = 'CAMION',
  RETRAIT = 'RETRAIT',
}

export interface Livreur {
  id: string;
  nom: string;
  telephone: string;
  vehicule?: string;
  modeLivraison: ModeLivraison;
  note?: number;
}

export interface Livraison {
  id: string;
  commandeId: string;
  livreur?: Livreur;
  modeLivraison: ModeLivraison;
  adresseDepart: string;
  adresseArrivee: string;
  dateEstimee?: string;
  dateLivraison?: string;
  statut: 'PLANIFIEE' | 'EN_COURS' | 'LIVREE' | 'ECHOUEE' | 'ANNULEE';
  positionActuelle?: { lat: number; lng: number };
  notes?: string;
  createdAt: string;
}
