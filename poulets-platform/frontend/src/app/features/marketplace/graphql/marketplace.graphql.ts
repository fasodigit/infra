import { gql } from 'apollo-angular';

// =============================================================================
// Annonces
// =============================================================================

export const GET_ANNONCES = gql`
  query GetAnnonces($filter: AnnonceFilter, $page: Int, $size: Int) {
    annonces(filter: $filter, page: $page, size: $size) {
      content {
        id
        race
        quantity
        currentWeight
        estimatedWeight
        targetDate
        pricePerKg
        pricePerUnit
        location
        description
        photos
        availabilityStart
        availabilityEnd
        veterinaryStatus
        halalCertified
        isGroupement
        status
        createdAt
        eleveur {
          id
          nom
          localisation
          note
          responseTime
          ponctualite
        }
      }
      totalElements
      totalPages
      currentPage
    }
  }
`;

export const GET_ANNONCE_BY_ID = gql`
  query GetAnnonceById($id: ID!) {
    annonce(id: $id) {
      id
      race
      quantity
      currentWeight
      estimatedWeight
      targetDate
      pricePerKg
      pricePerUnit
      location
      latitude
      longitude
      description
      photos
      availabilityStart
      availabilityEnd
      veterinaryStatus
      ficheSanitaireId
      halalCertified
      isGroupement
      groupementId
      status
      createdAt
      updatedAt
      eleveur {
        id
        nom
        prenom
        localisation
        note
        responseTime
        ponctualite
        totalVentes
        telephone
      }
    }
  }
`;

export const GET_SIMILAR_ANNONCES = gql`
  query GetSimilarAnnonces($annonceId: ID!, $limit: Int) {
    similarAnnonces(annonceId: $annonceId, limit: $limit) {
      id
      race
      quantity
      currentWeight
      pricePerKg
      location
      photos
      eleveur {
        id
        nom
        note
      }
    }
  }
`;

export const CREATE_ANNONCE = gql`
  mutation CreateAnnonce($input: CreateAnnonceInput!) {
    createAnnonce(input: $input) {
      id
      race
      quantity
      status
      createdAt
    }
  }
`;

// =============================================================================
// Besoins
// =============================================================================

export const GET_BESOINS = gql`
  query GetBesoins($filter: BesoinFilter, $page: Int, $size: Int) {
    besoins(filter: $filter, page: $page, size: $size) {
      content {
        id
        races
        quantity
        minimumWeight
        deliveryDate
        maxBudgetPerKg
        location
        frequency
        halalRequired
        veterinaryCertifiedRequired
        specialNotes
        status
        createdAt
        client {
          id
          nom
          localisation
        }
      }
      totalElements
      totalPages
      currentPage
    }
  }
`;

export const GET_BESOIN_BY_ID = gql`
  query GetBesoinById($id: ID!) {
    besoin(id: $id) {
      id
      races
      quantity
      minimumWeight
      deliveryDate
      maxBudgetPerKg
      location
      latitude
      longitude
      frequency
      recurringStartDate
      recurringEndDate
      dayOfWeekPreference
      halalRequired
      veterinaryCertifiedRequired
      specialNotes
      status
      createdAt
      updatedAt
      client {
        id
        nom
        localisation
      }
    }
  }
`;

export const CREATE_BESOIN = gql`
  mutation CreateBesoin($input: CreateBesoinInput!) {
    createBesoin(input: $input) {
      id
      races
      quantity
      status
      createdAt
    }
  }
`;

// =============================================================================
// Matching
// =============================================================================

export const GET_MATCHES_FOR_ELEVEUR = gql`
  query GetMatchesForEleveur($page: Int, $size: Int) {
    matchesForEleveur(page: $page, size: $size) {
      content {
        id
        besoin {
          id
          races
          quantity
          minimumWeight
          deliveryDate
          maxBudgetPerKg
          location
          frequency
          client {
            id
            nom
            localisation
          }
        }
        matchScore
        raceCompatibility
        weightFeasibility
        dateCompatibility
        proximity
        reputation
      }
      totalElements
      totalPages
      currentPage
    }
  }
`;

export const GET_MATCHES_FOR_CLIENT = gql`
  query GetMatchesForClient($page: Int, $size: Int) {
    matchesForClient(page: $page, size: $size) {
      content {
        id
        annonce {
          id
          race
          quantity
          currentWeight
          estimatedWeight
          pricePerKg
          location
          halalCertified
          veterinaryStatus
          eleveur {
            id
            nom
            note
            localisation
          }
        }
        matchScore
        raceCompatibility
        weightFeasibility
        dateCompatibility
        proximity
        reputation
      }
      totalElements
      totalPages
      currentPage
    }
  }
`;
