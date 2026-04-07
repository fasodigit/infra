export interface Aliment {
  id: string;
  nom: string;
  type: 'DEMARRAGE' | 'CROISSANCE' | 'FINITION' | 'PONTE';
  composition?: string;
  prixParKg: number;
  producteurId: string;
  producteurNom?: string;
  disponible: boolean;
  createdAt: string;
}

export interface FormuleAliment {
  id: string;
  nom: string;
  ingredients: IngredientFormule[];
  coutTotal?: number;
  description?: string;
}

export interface IngredientFormule {
  nom: string;
  pourcentage: number;
  coutParKg?: number;
}

export interface PlanAlimentaire {
  id: string;
  lotId: string;
  phases: PhaseAlimentaire[];
  coutEstime?: number;
  createdAt: string;
}

export interface PhaseAlimentaire {
  semaineDe: number;
  semaineA: number;
  alimentId?: string;
  alimentNom?: string;
  quantiteJournaliereParTete: number; // in grams
}
