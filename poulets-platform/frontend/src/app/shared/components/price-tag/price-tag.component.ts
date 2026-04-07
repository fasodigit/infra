import { Component, Input, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FcfaCurrencyPipe } from '../../pipes/currency.pipe';

@Component({
  selector: 'app-price-tag',
  standalone: true,
  imports: [CommonModule, FcfaCurrencyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span class="price-tag" [class.large]="size === 'large'" [class.small]="size === 'small'">
      <span class="price-amount">{{ value | fcfa }}</span>
      @if (unit) {
        <span class="price-unit">/ {{ unit }}</span>
      }
      @if (oldValue && oldValue > value) {
        <span class="price-old">{{ oldValue | fcfa }}</span>
      }
    </span>
  `,
  styles: [`
    .price-tag {
      display: inline-flex;
      align-items: baseline;
      gap: 4px;
    }

    .price-amount {
      font-weight: 700;
      color: var(--faso-primary-dark, #005005);
      font-size: 1.1rem;
    }

    .price-unit {
      font-size: 0.8rem;
      color: var(--faso-text-secondary);
    }

    .price-old {
      font-size: 0.85rem;
      color: var(--faso-text-secondary);
      text-decoration: line-through;
      margin-left: 4px;
    }

    .large .price-amount {
      font-size: 1.5rem;
    }

    .small .price-amount {
      font-size: 0.9rem;
    }

    .small .price-unit {
      font-size: 0.7rem;
    }
  `],
})
export class PriceTagComponent {
  @Input() value = 0;
  @Input() oldValue?: number;
  @Input() unit = '';
  @Input() size: 'small' | 'medium' | 'large' = 'medium';
}
