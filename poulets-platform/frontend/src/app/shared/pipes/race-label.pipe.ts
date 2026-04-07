import { Pipe, PipeTransform, inject } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';
import { Race } from '../models/poulet.model';

const RACE_TRANSLATION_KEYS: Record<string, string> = {
  [Race.LOCAL]: 'common.races.local',
  [Race.BICYCLETTE]: 'common.races.bicyclette',
  [Race.BRAHMA]: 'common.races.brahma',
  [Race.SUSSEX]: 'common.races.sussex',
  [Race.RHODE_ISLAND]: 'common.races.rhode_island',
  [Race.LEGHORN]: 'common.races.leghorn',
  [Race.COUCOU]: 'common.races.coucou',
  [Race.PINTADE]: 'common.races.pintade',
  [Race.DINDE]: 'common.races.dinde',
  [Race.MIXED]: 'common.races.mixed',
};

/**
 * Transforms a Race enum value into a human-readable label.
 * Uses i18n to support FR/EN.
 */
@Pipe({
  name: 'raceLabel',
  standalone: true,
  pure: false,
})
export class RaceLabelPipe implements PipeTransform {
  private readonly translate = inject(TranslateService);

  transform(value: string | Race | null | undefined): string {
    if (!value) {
      return '';
    }

    const key = RACE_TRANSLATION_KEYS[value];
    if (key) {
      return this.translate.instant(key);
    }
    // Fallback: capitalize the value
    return value.charAt(0) + value.slice(1).toLowerCase().replace(/_/g, ' ');
  }
}
