export enum Frequence {
  HEBDOMADAIRE = 'HEBDOMADAIRE',
  BIMENSUEL = 'BIMENSUEL',
  MENSUEL = 'MENSUEL',
  TRIMESTRIEL = 'TRIMESTRIEL',
}

export interface ContratRecurrent {
  id: string;
  clientId: string;
  clientNom?: string;
  eleveurId: string;
  eleveurNom?: string;
  race: string;
  quantiteParLivraison: number;
  prixUnitaire: number;
  frequence: Frequence;
  dateDebut: string;
  dateFin?: string;
  prochaineLivraison?: string;
  statut: 'ACTIF' | 'EN_ATTENTE' | 'SUSPENDU' | 'TERMINE' | 'ANNULE';
  totalLivraisons?: number;
  livraisonsEffectuees?: number;
  createdAt: string;
  updatedAt?: string;
}
