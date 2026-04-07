export interface Vaccination {
  id: string;
  nomVaccin: string;
  dateAdministration: string;
  administrePar: string;
  prochaineDose?: string;
  lotId?: string;
  observations?: string;
}

export interface Traitement {
  id: string;
  nomTraitement: string;
  diagnostic: string;
  dateDebut: string;
  dateFin?: string;
  duree?: number; // in days
  prescritPar: string;
  lotId?: string;
  observations?: string;
}

export type StatutSanitaire = 'SAIN' | 'EN_TRAITEMENT' | 'QUARANTAINE';

export interface FicheSanitaire {
  id: string;
  lotId: string;
  lotNom?: string;
  statut: StatutSanitaire;
  vaccinations: Vaccination[];
  traitements: Traitement[];
  derniereVisite?: string;
  prochaineVisite?: string;
  veterinaire?: string;
  observations?: string;
  createdAt: string;
  updatedAt?: string;
}
