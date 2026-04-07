import { Pipe, PipeTransform, inject } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';

/**
 * Formats a date as a relative string.
 * Example: "2024-01-01" -> "il y a 3 jours" (fr) or "3 days ago" (en)
 */
@Pipe({
  name: 'relativeDate',
  standalone: true,
  pure: false,
})
export class RelativeDatePipe implements PipeTransform {
  private readonly translate = inject(TranslateService);

  transform(value: string | Date | null | undefined): string {
    if (!value) {
      return '';
    }

    const date = value instanceof Date ? value : new Date(value);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffSeconds = Math.floor(Math.abs(diffMs) / 1000);
    const diffMinutes = Math.floor(diffSeconds / 60);
    const diffHours = Math.floor(diffMinutes / 60);
    const diffDays = Math.floor(diffHours / 24);
    const diffWeeks = Math.floor(diffDays / 7);
    const diffMonths = Math.floor(diffDays / 30);

    const isFuture = diffMs < 0;
    const lang = this.translate.currentLang || 'fr';

    if (diffSeconds < 60) {
      return lang === 'fr' ? "à l'instant" : 'just now';
    }

    let unit: string;
    let count: number;

    if (diffMinutes < 60) {
      count = diffMinutes;
      unit = lang === 'fr' ? (count > 1 ? 'minutes' : 'minute') : (count > 1 ? 'minutes' : 'minute');
    } else if (diffHours < 24) {
      count = diffHours;
      unit = lang === 'fr' ? (count > 1 ? 'heures' : 'heure') : (count > 1 ? 'hours' : 'hour');
    } else if (diffDays < 7) {
      count = diffDays;
      unit = lang === 'fr' ? (count > 1 ? 'jours' : 'jour') : (count > 1 ? 'days' : 'day');
    } else if (diffWeeks < 5) {
      count = diffWeeks;
      unit = lang === 'fr' ? (count > 1 ? 'semaines' : 'semaine') : (count > 1 ? 'weeks' : 'week');
    } else {
      count = diffMonths;
      unit = lang === 'fr' ? 'mois' : (count > 1 ? 'months' : 'month');
    }

    if (isFuture) {
      return lang === 'fr' ? `dans ${count} ${unit}` : `in ${count} ${unit}`;
    }
    return lang === 'fr' ? `il y a ${count} ${unit}` : `${count} ${unit} ago`;
  }
}
