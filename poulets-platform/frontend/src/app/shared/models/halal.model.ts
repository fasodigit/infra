export interface Abattoir {
  id: string;
  nom: string;
  adresse: string;
  telephone?: string;
  certifie: boolean;
  capaciteJournaliere?: number;
}

export interface CertificationHalal {
  id: string;
  numero: string;
  lotId?: string;
  abattoir: Abattoir;
  dateCertification: string;
  dateExpiration: string;
  statut: 'VALIDE' | 'EXPIRE' | 'EN_ATTENTE' | 'REJETE';
  inspecteur?: string;
  observations?: string;
  createdAt: string;
}
