/**
 * Calendar domain models for events and planning views.
 */

export interface CalendarEvent {
  id: string;
  title: string;
  description?: string;
  start: string;
  end?: string;
  allDay: boolean;
  type: CalendarEventType;
  color: string;
  relatedId?: string;
  race?: string;
  quantity?: number;
  location?: string;
  metadata?: Record<string, unknown>;
}

export type CalendarEventType =
  | 'LOT_DISPONIBLE'
  | 'LIVRAISON'
  | 'CONTRAT_LIVRAISON'
  | 'DEADLINE_POIDS'
  | 'VETERINAIRE';

export const EVENT_COLORS: Record<CalendarEventType, string> = {
  LOT_DISPONIBLE: '#4caf50',
  LIVRAISON: '#2196f3',
  CONTRAT_LIVRAISON: '#ff9800',
  DEADLINE_POIDS: '#f44336',
  VETERINAIRE: '#9c27b0',
};

export type CalendarViewMode = 'month' | 'week' | 'day';

export interface CalendarFilter {
  myEventsOnly: boolean;
  allMarketplace: boolean;
  specificRace?: string;
  eventTypes: CalendarEventType[];
}

export interface SupplyDemandWeek {
  weekStart: string;
  weekEnd: string;
  weekLabel: string;
  supply: number;
  demand: number;
  gap: number;
}

export interface PlanningData {
  race: string;
  weeks: SupplyDemandWeek[];
}

export interface PlanningFilter {
  race?: string;
  location?: string;
  dateFrom: string;
  dateTo: string;
}
