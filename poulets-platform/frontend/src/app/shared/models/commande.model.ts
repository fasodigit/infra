export enum CommandeStatus {
  EN_ATTENTE = 'EN_ATTENTE',
  CONFIRMEE = 'CONFIRMEE',
  EN_PREPARATION = 'EN_PREPARATION',
  EN_LIVRAISON = 'EN_LIVRAISON',
  LIVREE = 'LIVREE',
  ANNULEE = 'ANNULEE',
}

export interface CommandeItem {
  id: string;
  pouletId?: string;
  annonceId?: string;
  race: string;
  quantite: number;
  prixUnitaire: number;
  poidsMoyen?: number;
}

export interface Commande {
  id: string;
  numero: string;
  clientId: string;
  clientNom?: string;
  eleveurId: string;
  eleveurNom?: string;
  items: CommandeItem[];
  statut: CommandeStatus;
  prixTotal: number;
  adresseLivraison: string;
  telephone: string;
  notes?: string;
  livraisonId?: string;
  createdAt: string;
  updatedAt?: string;
}
