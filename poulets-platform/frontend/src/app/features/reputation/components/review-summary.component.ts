// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input, computed, signal, effect } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { ReviewStats } from '@shared/models/reputation.models';

@Component({
  selector: 'app-review-summary',
  standalone: true,
  imports: [CommonModule, MatIconModule, DecimalPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (stats) {
      <div class="summary">
        <div class="score">
          <div class="avg">{{ stats.average | number:'1.1-1' }}</div>
          <div class="stars" aria-hidden="true">
            @for (i of [1,2,3,4,5]; track i) {
              <mat-icon [class.filled]="i <= stats.average">
                {{ i <= stats.average ? 'star' : (i - 0.5 <= stats.average ? 'star_half' : 'star_border') }}
              </mat-icon>
            }
          </div>
          <div class="total">{{ stats.total }} avis</div>
        </div>

        <div class="bars">
          @for (i of [4,3,2,1,0]; track i) {
            <div class="bar-row">
              <span class="label">{{ i + 1 }}★</span>
              <div class="bar"><div class="fill" [style.width.%]="percent(i)"></div></div>
              <span class="count">{{ stats.distribution[i] }}</span>
            </div>
          }
        </div>
      </div>
    }
  `,
  styles: [`
    :host { display: block; }
    .summary {
      display: grid;
      grid-template-columns: auto 1fr;
      gap: var(--faso-space-6);
      align-items: center;
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
    }
    .score { text-align: center; min-width: 120px; }
    .avg {
      font-size: 3rem;
      font-weight: 700;
      color: var(--faso-accent-700);
      line-height: 1;
    }
    .stars {
      display: inline-flex;
      gap: 2px;
      margin-top: 4px;
      color: #CBD5E1;
    }
    .stars .filled { color: var(--faso-accent-500); }
    .stars mat-icon { font-size: 20px; width: 20px; height: 20px; }
    .total {
      font-size: var(--faso-text-sm);
      color: var(--faso-text-muted);
      margin-top: 4px;
    }

    .bars { display: flex; flex-direction: column; gap: 6px; }
    .bar-row {
      display: grid;
      grid-template-columns: 32px 1fr 36px;
      gap: 8px;
      align-items: center;
      font-size: var(--faso-text-sm);
    }
    .bar-row .label {
      color: var(--faso-text-muted);
      text-align: right;
    }
    .bar {
      height: 8px;
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-pill);
      overflow: hidden;
    }
    .fill {
      height: 100%;
      background: var(--faso-accent-500);
      border-radius: inherit;
      transition: width var(--faso-duration-slow) var(--faso-ease-standard);
    }
    .count {
      color: var(--faso-text-muted);
      text-align: left;
    }

    @media (max-width: 639px) {
      .summary { grid-template-columns: 1fr; }
    }
  `],
})
export class ReviewSummaryComponent {
  @Input({ required: true }) stats!: ReviewStats;

  percent(idx: number): number {
    if (!this.stats?.total) return 0;
    return Math.round((this.stats.distribution[idx] / this.stats.total) * 100);
  }
}
