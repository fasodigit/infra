import { Pipe, PipeTransform, inject } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';

const STATUS_TRANSLATION_KEYS: Record<string, string> = {
  DISPONIBLE: 'common.status.available',
  RESERVE: 'common.status.reserved',
  VENDU: 'common.status.sold',
  EN_ATTENTE: 'common.status.pending',
  CONFIRMEE: 'common.status.confirmed',
  EN_PREPARATION: 'common.status.pending',
  EN_LIVRAISON: 'common.status.in_delivery',
  LIVREE: 'common.status.delivered',
  ANNULEE: 'common.status.cancelled',
  ACTIF: 'common.status.active',
  ACTIVE: 'common.status.active',
  INACTIF: 'common.status.inactive',
  INACTIVE: 'common.status.inactive',
  EXPIRE: 'common.status.expired',
  EXPIREE: 'common.status.expired',
  TERMINE: 'common.status.completed',
  SUSPENDU: 'common.status.inactive',
  PLANIFIEE: 'calendar.scheduled',
  EN_COURS: 'calendar.in_progress',
  SAIN: 'veterinary.status_healthy',
  EN_TRAITEMENT: 'veterinary.status_treatment',
  QUARANTAINE: 'veterinary.status_quarantine',
  VALIDE: 'halal.status_valid',
  REJETE: 'common.status.cancelled',
  SATISFAIT: 'common.status.completed',
  ECHOUEE: 'common.status.cancelled',
};

export const STATUS_COLORS: Record<string, string> = {
  DISPONIBLE: '#4caf50',
  RESERVE: '#ff9800',
  VENDU: '#9e9e9e',
  EN_ATTENTE: '#ff9800',
  CONFIRMEE: '#2196f3',
  EN_PREPARATION: '#03a9f4',
  EN_LIVRAISON: '#ff9800',
  LIVREE: '#4caf50',
  ANNULEE: '#f44336',
  ACTIF: '#4caf50',
  ACTIVE: '#4caf50',
  INACTIF: '#9e9e9e',
  INACTIVE: '#9e9e9e',
  EXPIRE: '#f44336',
  EXPIREE: '#f44336',
  TERMINE: '#4caf50',
  SUSPENDU: '#ff9800',
  PLANIFIEE: '#2196f3',
  EN_COURS: '#ff9800',
  SAIN: '#4caf50',
  EN_TRAITEMENT: '#ff9800',
  QUARANTAINE: '#f44336',
  VALIDE: '#4caf50',
  REJETE: '#f44336',
  SATISFAIT: '#4caf50',
  ECHOUEE: '#f44336',
};

/**
 * Transforms a status enum value into a human-readable label.
 */
@Pipe({
  name: 'statusLabel',
  standalone: true,
  pure: false,
})
export class StatusLabelPipe implements PipeTransform {
  private readonly translate = inject(TranslateService);

  transform(value: string | null | undefined): string {
    if (!value) {
      return '';
    }

    const key = STATUS_TRANSLATION_KEYS[value];
    if (key) {
      return this.translate.instant(key);
    }
    return value.charAt(0) + value.slice(1).toLowerCase().replace(/_/g, ' ');
  }
}
