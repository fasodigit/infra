// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';

export type TrendDirection = 'up' | 'down' | 'flat';

@Component({
  selector: 'app-kpi-card',
  standalone: true,
  imports: [CommonModule, MatIconModule, DecimalPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <article class="card">
      <header>
        <span class="icon-wrap"><mat-icon>{{ icon }}</mat-icon></span>
        @if (trend !== undefined) {
          <span class="trend" [class.is-up]="direction === 'up'" [class.is-down]="direction === 'down'">
            <mat-icon>{{ arrowIcon }}</mat-icon>
            {{ trend | number:'1.0-1' }}%
          </span>
        }
      </header>
      <div class="value">
        <strong>{{ value }}</strong>
        @if (unit) { <span>{{ unit }}</span> }
      </div>
      <div class="label">{{ label }}</div>
      @if (sublabel) { <div class="sub">{{ sublabel }}</div> }
    </article>
  `,
  styles: [`
    :host { display: block; }
    .card {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-5);
      box-shadow: var(--faso-shadow-xs);
      height: 100%;
    }
    header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      margin-bottom: var(--faso-space-3);
    }
    .icon-wrap {
      display: inline-flex;
      width: 40px;
      height: 40px;
      border-radius: 10px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      align-items: center;
      justify-content: center;
    }
    .icon-wrap mat-icon { font-size: 22px; width: 22px; height: 22px; }

    .trend {
      display: inline-flex;
      align-items: center;
      gap: 2px;
      padding: 2px 8px;
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      color: var(--faso-text-muted);
    }
    .trend mat-icon { font-size: 14px; width: 14px; height: 14px; }
    .trend.is-up   { background: var(--faso-success-bg); color: var(--faso-success); }
    .trend.is-down { background: var(--faso-danger-bg);  color: var(--faso-danger); }

    .value {
      display: flex;
      align-items: baseline;
      gap: 4px;
    }
    .value strong {
      font-size: var(--faso-text-4xl);
      font-weight: var(--faso-weight-bold);
      color: var(--faso-text);
      line-height: 1.1;
    }
    .value span {
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }
    .label {
      margin-top: var(--faso-space-1);
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-medium);
    }
    .sub {
      margin-top: 2px;
      color: var(--faso-text-subtle);
      font-size: var(--faso-text-xs);
    }
  `],
})
export class KpiCardComponent {
  @Input({ required: true }) icon!: string;
  @Input({ required: true }) label!: string;
  @Input({ required: true }) value!: string | number;
  @Input() unit?: string;
  @Input() sublabel?: string;
  @Input() trend?: number;
  @Input() direction: TrendDirection = 'flat';

  get arrowIcon(): string {
    return this.direction === 'up' ? 'arrow_upward'
         : this.direction === 'down' ? 'arrow_downward'
         : 'remove';
  }
}
