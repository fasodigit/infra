import { gql } from 'apollo-angular';

export const GET_CONTRACTS = gql`
  query GetContracts($filter: ContractFilterInput, $page: Int, $size: Int) {
    contracts(filter: $filter, page: $page, size: $size) {
      content {
        id
        partnerId
        partnerName
        partnerRole
        race
        quantityPerDelivery
        minimumWeight
        pricePerKg
        priceType
        frequency
        startDate
        endDate
        duration
        status
        signedByInitiator
        signedByPartner
        halalRequired
        veterinaryCertificationRequired
        createdAt
      }
      totalElements
      totalPages
      currentPage
    }
  }
`;

export const GET_CONTRACT_BY_ID = gql`
  query GetContractById($id: ID!) {
    contract(id: $id) {
      id
      partnerId
      partnerName
      partnerRole
      race
      quantityPerDelivery
      minimumWeight
      pricePerKg
      priceType
      frequency
      dayPreference
      startDate
      endDate
      duration
      advancePaymentPercent
      penaltyLateDelivery
      penaltyUnderWeight
      halalRequired
      veterinaryCertificationRequired
      signedByInitiator
      signedByPartner
      status
      createdAt
      updatedAt
      deliveries {
        id
        scheduledDate
        actualDate
        status
        quantityDelivered
        averageWeight
        notes
      }
    }
  }
`;

export const GET_CONTRACT_PERFORMANCE = gql`
  query GetContractPerformance($contractId: ID!) {
    contractPerformance(contractId: $contractId) {
      totalDeliveries
      completedDeliveries
      onTimePercent
      averageWeightVsContracted
      nextDeliveryDate
      daysUntilNextDelivery
    }
  }
`;

export const CREATE_CONTRACT = gql`
  mutation CreateContract($input: CreateContractInput!) {
    createContract(input: $input) {
      id
      status
      createdAt
    }
  }
`;

export const SIGN_CONTRACT = gql`
  mutation SignContract($contractId: ID!) {
    signContract(contractId: $contractId) {
      id
      signedByInitiator
      signedByPartner
      status
    }
  }
`;

export const RENEW_CONTRACT = gql`
  mutation RenewContract($contractId: ID!, $newDuration: String!) {
    renewContract(contractId: $contractId, newDuration: $newDuration) {
      id
      status
      startDate
      endDate
    }
  }
`;

export const TERMINATE_CONTRACT = gql`
  mutation TerminateContract($contractId: ID!, $reason: String) {
    terminateContract(contractId: $contractId, reason: $reason) {
      id
      status
    }
  }
`;

export const SEARCH_PARTNERS = gql`
  query SearchPartners($query: String!, $role: String) {
    searchPartners(query: $query, role: $role) {
      id
      nom
      prenom
      role
      localisation
      note
    }
  }
`;
