import { gql } from 'apollo-angular';

// =============================================================================
// GraphQL Queries
// =============================================================================

export const GET_POULETS = gql`
  query GetPoulets($filter: PouletFilter, $page: Int, $size: Int) {
    poulets(filter: $filter, page: $page, size: $size) {
      content {
        id
        race
        age
        poids
        prix
        statut
        description
        photos
        eleveur {
          id
          nom
          localisation
          note
        }
        createdAt
      }
      totalElements
      totalPages
      currentPage
    }
  }
`;

export const GET_POULET_BY_ID = gql`
  query GetPouletById($id: ID!) {
    poulet(id: $id) {
      id
      race
      age
      poids
      prix
      statut
      description
      photos
      alimentation
      vaccinations
      certificats
      eleveur {
        id
        nom
        prenom
        telephone
        localisation
        note
        totalVentes
      }
      createdAt
      updatedAt
    }
  }
`;

export const GET_MES_POULETS = gql`
  query GetMesPoulets($page: Int, $size: Int) {
    mesPoulets(page: $page, size: $size) {
      content {
        id
        race
        age
        poids
        prix
        statut
        description
        photos
        createdAt
      }
      totalElements
      totalPages
      currentPage
    }
  }
`;

export const GET_MES_COMMANDES = gql`
  query GetMesCommandes($page: Int, $size: Int) {
    mesCommandes(page: $page, size: $size) {
      content {
        id
        poulet {
          id
          race
          poids
          prix
        }
        statut
        quantite
        prixTotal
        adresseLivraison
        createdAt
      }
      totalElements
      totalPages
      currentPage
    }
  }
`;

export const GET_ELEVEUR_STATS = gql`
  query GetEleveurStats {
    eleveurStats {
      totalPoulets
      pouletsDisponibles
      pouletsVendus
      chiffreAffaires
      commandesEnCours
      noteMoyenne
    }
  }
`;

// =============================================================================
// GraphQL Mutations
// =============================================================================

export const CREATE_POULET = gql`
  mutation CreatePoulet($input: CreatePouletInput!) {
    createPoulet(input: $input) {
      id
      race
      age
      poids
      prix
      statut
      description
    }
  }
`;

export const UPDATE_POULET = gql`
  mutation UpdatePoulet($id: ID!, $input: UpdatePouletInput!) {
    updatePoulet(id: $id, input: $input) {
      id
      race
      age
      poids
      prix
      statut
      description
    }
  }
`;

export const DELETE_POULET = gql`
  mutation DeletePoulet($id: ID!) {
    deletePoulet(id: $id)
  }
`;

export const PASSER_COMMANDE = gql`
  mutation PasserCommande($input: CommandeInput!) {
    passerCommande(input: $input) {
      id
      statut
      prixTotal
      createdAt
    }
  }
`;

// =============================================================================
// TypeScript interfaces matching the GraphQL schema
// =============================================================================

export interface Poulet {
  id: string;
  race: string;
  age: number;
  poids: number;
  prix: number;
  statut: 'DISPONIBLE' | 'RESERVE' | 'VENDU';
  description: string;
  photos: string[];
  alimentation?: string;
  vaccinations?: string[];
  certificats?: string[];
  eleveur: Eleveur;
  createdAt: string;
  updatedAt?: string;
}

export interface Eleveur {
  id: string;
  nom: string;
  prenom?: string;
  telephone?: string;
  localisation: string;
  note: number;
  totalVentes?: number;
}

export interface Commande {
  id: string;
  poulet: Poulet;
  statut: 'EN_ATTENTE' | 'CONFIRMEE' | 'EN_LIVRAISON' | 'LIVREE' | 'ANNULEE';
  quantite: number;
  prixTotal: number;
  adresseLivraison: string;
  createdAt: string;
}

export interface PouletFilter {
  race?: string;
  prixMin?: number;
  prixMax?: number;
  poidsMin?: number;
  poidsMax?: number;
  localisation?: string;
  statut?: string;
}

export interface Page<T> {
  content: T[];
  totalElements: number;
  totalPages: number;
  currentPage: number;
}

export interface EleveurStats {
  totalPoulets: number;
  pouletsDisponibles: number;
  pouletsVendus: number;
  chiffreAffaires: number;
  commandesEnCours: number;
  noteMoyenne: number;
}

export interface CreatePouletInput {
  race: string;
  age: number;
  poids: number;
  prix: number;
  description: string;
  alimentation?: string;
  vaccinations?: string[];
}

export interface UpdatePouletInput {
  race?: string;
  age?: number;
  poids?: number;
  prix?: number;
  description?: string;
  statut?: string;
}

export interface CommandeInput {
  pouletId: string;
  quantite: number;
  adresseLivraison: string;
  telephone: string;
  notes?: string;
}
