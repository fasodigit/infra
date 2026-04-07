import { Pipe, PipeTransform } from '@angular/core';

/**
 * Formats a number as weight in kg.
 * Example: 2.5 -> "2,5 kg"
 */
@Pipe({
  name: 'weight',
  standalone: true,
})
export class WeightPipe implements PipeTransform {
  transform(value: number | null | undefined, decimals: number = 1): string {
    if (value == null || isNaN(value)) {
      return '0 kg';
    }

    const formatted = value
      .toFixed(decimals)
      .replace('.', ',');

    return `${formatted} kg`;
  }
}
