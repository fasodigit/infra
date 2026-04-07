/**
 * Contract domain models for recurring supply contracts.
 */

export interface Contract {
  id: string;
  partnerId: string;
  partnerName: string;
  partnerRole: 'eleveur' | 'client';
  race: string;
  quantityPerDelivery: number;
  minimumWeight: number;
  pricePerKg: number;
  priceType: ContractPriceType;
  frequency: ContractFrequency;
  dayPreference?: number;
  startDate: string;
  endDate: string;
  duration: ContractDuration;
  advancePaymentPercent: number;
  penaltyLateDelivery: number;
  penaltyUnderWeight: number;
  halalRequired: boolean;
  veterinaryCertificationRequired: boolean;
  signedByInitiator: boolean;
  signedByPartner: boolean;
  status: ContractStatus;
  deliveries: ContractDelivery[];
  createdAt: string;
  updatedAt: string;
}

export type ContractStatus = 'BROUILLON' | 'EN_ATTENTE' | 'ACTIF' | 'SUSPENDU' | 'EXPIRE' | 'RESILIE';

export type ContractPriceType = 'FIXE' | 'INDEXE';

export type ContractFrequency = 'HEBDOMADAIRE' | 'BI_MENSUEL' | 'MENSUEL';

export type ContractDuration = '3_MOIS' | '6_MOIS' | '12_MOIS';

export interface ContractDelivery {
  id: string;
  contractId: string;
  scheduledDate: string;
  actualDate?: string;
  status: DeliveryStatus;
  quantityDelivered?: number;
  averageWeight?: number;
  notes?: string;
}

export type DeliveryStatus = 'PLANIFIE' | 'A_TEMPS' | 'EN_RETARD' | 'ANNULE';

export interface ContractPerformance {
  totalDeliveries: number;
  completedDeliveries: number;
  onTimePercent: number;
  averageWeightVsContracted: number;
  nextDeliveryDate?: string;
  daysUntilNextDelivery?: number;
}

export interface CreateContractInput {
  partnerId: string;
  race: string;
  quantityPerDelivery: number;
  minimumWeight: number;
  pricePerKg: number;
  priceType: ContractPriceType;
  frequency: ContractFrequency;
  dayPreference?: number;
  startDate: string;
  duration: ContractDuration;
  advancePaymentPercent: number;
  penaltyLateDelivery: number;
  penaltyUnderWeight: number;
  halalRequired: boolean;
  veterinaryCertificationRequired: boolean;
}

export interface ContractFilter {
  status?: ContractStatus;
  partnerId?: string;
  race?: string;
}

/** Helper to get contract end date from start + duration */
export function computeEndDate(startDate: string, duration: ContractDuration): Date {
  const start = new Date(startDate);
  switch (duration) {
    case '3_MOIS':
      start.setMonth(start.getMonth() + 3);
      break;
    case '6_MOIS':
      start.setMonth(start.getMonth() + 6);
      break;
    case '12_MOIS':
      start.setFullYear(start.getFullYear() + 1);
      break;
  }
  return start;
}
