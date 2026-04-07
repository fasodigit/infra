import { Pipe, PipeTransform } from '@angular/core';

/**
 * Formats a number as FCFA currency.
 * Example: 2500 -> "2 500 FCFA"
 */
@Pipe({
  name: 'fcfa',
  standalone: true,
})
export class FcfaCurrencyPipe implements PipeTransform {
  transform(value: number | null | undefined, showSymbol: boolean = true): string {
    if (value == null || isNaN(value)) {
      return showSymbol ? '0 FCFA' : '0';
    }

    const formatted = Math.round(value)
      .toString()
      .replace(/\B(?=(\d{3})+(?!\d))/g, ' ');

    return showSymbol ? `${formatted} FCFA` : formatted;
  }
}
