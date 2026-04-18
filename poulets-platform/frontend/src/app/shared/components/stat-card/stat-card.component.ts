// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';

export type StatStatus = 'healthy' | 'degraded' | 'critical' | 'neutral';

@Component({
  selector: 'app-stat-card',
  standalone: true,
  imports: [CommonModule, DecimalPipe, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <article class="stat" [class]="'stat--' + status">
      <header>
        <span class="icon-wrap"><mat-icon>{{ icon }}</mat-icon></span>
        @if (status !== 'neutral') {
          <span class="status-dot"></span>
        }
      </header>
      <div class="value">
        <strong>
          @if (typeof(value) === 'number') { {{ value | number:'1.0-0' }} }
          @else { {{ value }} }
        </strong>
        @if (unit) { <span class="unit">{{ unit }}</span> }
      </div>
      <div class="label">{{ label }}</div>
      @if (sublabel) { <div class="sublabel">{{ sublabel }}</div> }
    </article>
  `,
  styles: [`
    :host { display: block; }

    .stat {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-5);
      box-shadow: var(--faso-shadow-xs);
      height: 100%;
      transition: border-color var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .stat--healthy  { border-left: 4px solid var(--faso-success); }
    .stat--degraded { border-left: 4px solid var(--faso-warning); }
    .stat--critical { border-left: 4px solid var(--faso-danger); }

    header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      margin-bottom: var(--faso-space-3);
    }
    .icon-wrap {
      display: inline-flex;
      width: 40px; height: 40px;
      border-radius: 10px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      align-items: center;
      justify-content: center;
    }
    .icon-wrap mat-icon { font-size: 22px; width: 22px; height: 22px; }

    .status-dot {
      width: 12px; height: 12px;
      border-radius: 50%;
      background: currentColor;
    }
    .stat--healthy  .status-dot { color: var(--faso-success); box-shadow: 0 0 0 3px var(--faso-success-bg); }
    .stat--degraded .status-dot { color: var(--faso-warning); box-shadow: 0 0 0 3px var(--faso-warning-bg); }
    .stat--critical .status-dot {
      color: var(--faso-danger);
      box-shadow: 0 0 0 3px var(--faso-danger-bg);
      animation: pulse 1.8s ease-in-out infinite;
    }
    @keyframes pulse {
      0%, 100% { opacity: 1; }
      50%      { opacity: 0.35; }
    }
    @media (prefers-reduced-motion: reduce) {
      .stat--critical .status-dot { animation: none; }
    }

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
    .unit { color: var(--faso-text-muted); font-size: var(--faso-text-sm); }
    .label {
      margin-top: var(--faso-space-1);
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-medium);
    }
    .sublabel {
      margin-top: 2px;
      color: var(--faso-text-subtle);
      font-size: var(--faso-text-xs);
    }
  `],
})
export class StatCardComponent {
  @Input({ required: true }) icon!: string;
  @Input({ required: true }) label!: string;
  @Input({ required: true }) value!: string | number;
  @Input() unit?: string;
  @Input() sublabel?: string;
  @Input() status: StatStatus = 'neutral';

  typeof(v: unknown): string { return typeof v; }
}
