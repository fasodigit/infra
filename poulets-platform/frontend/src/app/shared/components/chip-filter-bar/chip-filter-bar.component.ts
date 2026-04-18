import { ChangeDetectionStrategy, Component, EventEmitter, Input, Output } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';

export interface ActiveChip { key: string; label: string; }

@Component({
  selector: 'app-chip-filter-bar',
  standalone: true,
  imports: [CommonModule, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (chips.length > 0) {
      <div class="bar" role="region" aria-label="Filtres actifs">
        @for (chip of chips; track chip.key) {
          <button type="button" class="chip" (click)="remove.emit(chip.key)">
            <span>{{ chip.label }}</span>
            <mat-icon>close</mat-icon>
          </button>
        }
        <button type="button" class="clear" (click)="clearAll.emit()">
          Tout effacer
        </button>
      </div>
    }
  `,
  styles: [`
    :host { display: block; }
    .bar {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      padding: var(--faso-space-3) 0;
    }
    .chip {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      padding: 6px 6px 6px 12px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      border: 1px solid var(--faso-primary-200);
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-medium);
      cursor: pointer;
    }
    .chip mat-icon {
      font-size: 16px; width: 16px; height: 16px;
    }
    .chip:hover { background: var(--faso-primary-100); }

    .clear {
      background: transparent;
      border: none;
      color: var(--faso-text-muted);
      cursor: pointer;
      text-decoration: underline;
      font-size: var(--faso-text-sm);
      padding: 6px 8px;
    }
  `],
})
export class ChipFilterBarComponent {
  @Input() chips: ActiveChip[] = [];
  @Output() remove = new EventEmitter<string>();
  @Output() clearAll = new EventEmitter<void>();
}
