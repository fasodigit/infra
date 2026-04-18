// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input, computed, signal } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';

export interface GrowthPoint {
  /** Age in days */
  age: number;
  /** Weight in grams (actual measurement) */
  weight: number;
}

/** FAO reference curve for Ross broiler — approximate grams by day. */
const FAO_REFERENCE: GrowthPoint[] = [
  { age: 0,  weight: 42 },
  { age: 7,  weight: 190 },
  { age: 14, weight: 475 },
  { age: 21, weight: 900 },
  { age: 28, weight: 1420 },
  { age: 35, weight: 2000 },
  { age: 42, weight: 2570 },
  { age: 49, weight: 3080 },
];

@Component({
  selector: 'app-growth-chart',
  standalone: true,
  imports: [CommonModule, DecimalPipe, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="chart-wrap">
      <header>
        <div>
          <h3>Courbe de croissance</h3>
          <p>Lot <strong>{{ lotLabel }}</strong> · comparé à la référence FAO Ross</p>
        </div>
        <div class="legend">
          <span class="key actual"></span> <span>Votre lot</span>
          <span class="key ref"></span> <span>Référence FAO</span>
        </div>
      </header>

      <svg
        class="svg"
        [attr.viewBox]="'0 0 ' + width + ' ' + height"
        [attr.aria-label]="'Courbe de croissance du lot ' + lotLabel"
        role="img"
      >
        <defs>
          <linearGradient id="growthFill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color="#2E7D32" stop-opacity="0.25"/>
            <stop offset="100%" stop-color="#2E7D32" stop-opacity="0"/>
          </linearGradient>
        </defs>

        <!-- Y grid lines -->
        @for (y of yGrid(); track y.value) {
          <line [attr.x1]="padL" [attr.x2]="width - padR"
                [attr.y1]="y.pos" [attr.y2]="y.pos"
                stroke="#E5E7EB" stroke-width="1"/>
          <text [attr.x]="padL - 8" [attr.y]="y.pos + 4" text-anchor="end"
                font-size="10" fill="#64748B">{{ y.value }} g</text>
        }

        <!-- X axis labels -->
        @for (x of xTicks(); track x.value) {
          <text [attr.x]="x.pos" [attr.y]="height - padB + 16"
                text-anchor="middle" font-size="10" fill="#64748B">
            J{{ x.value }}
          </text>
        }

        <!-- Reference line -->
        <path
          [attr.d]="refPath()"
          fill="none"
          stroke="#FF8F00"
          stroke-width="2"
          stroke-dasharray="5,4"
        />

        <!-- Actual filled area -->
        <path [attr.d]="areaPath()" fill="url(#growthFill)"/>
        <!-- Actual line -->
        <path
          [attr.d]="linePath()"
          fill="none"
          stroke="#2E7D32"
          stroke-width="2.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        />

        <!-- Actual points -->
        @for (p of scaledPoints(); track p.age) {
          <circle [attr.cx]="p.x" [attr.cy]="p.y" r="4" fill="#2E7D32" stroke="#FFFFFF" stroke-width="1.5"/>
        }
      </svg>

      @if (summary(); as s) {
        <div class="summary">
          <div>
            <span>Dernière pesée</span>
            <strong>{{ s.lastWeight | number:'1.0-0' }} g à J{{ s.lastAge }}</strong>
          </div>
          <div>
            <span>Référence à cet âge</span>
            <strong>{{ s.reference | number:'1.0-0' }} g</strong>
          </div>
          <div [class.positive]="s.delta >= 0" [class.negative]="s.delta < 0">
            <span>Écart</span>
            <strong>
              <mat-icon>{{ s.delta >= 0 ? 'trending_up' : 'trending_down' }}</mat-icon>
              {{ s.delta > 0 ? '+' : '' }}{{ s.delta | number:'1.0-0' }} g
              ({{ s.deltaPercent > 0 ? '+' : '' }}{{ s.deltaPercent | number:'1.0-1' }}%)
            </strong>
          </div>
        </div>
      }
    </div>
  `,
  styles: [`
    :host { display: block; }

    .chart-wrap {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-5);
    }

    header {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-3);
      flex-wrap: wrap;
    }
    h3 { margin: 0; font-size: var(--faso-text-lg); font-weight: var(--faso-weight-semibold); }
    header p { margin: 2px 0 0; color: var(--faso-text-muted); font-size: var(--faso-text-sm); }

    .legend {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: var(--faso-text-xs);
      color: var(--faso-text-muted);
    }
    .key {
      display: inline-block;
      width: 16px;
      height: 3px;
      border-radius: 2px;
    }
    .key.actual { background: var(--faso-primary-600); }
    .key.ref    { background: var(--faso-accent-700); }

    .svg {
      width: 100%;
      height: auto;
      display: block;
    }

    .summary {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
      gap: var(--faso-space-3);
      padding-top: var(--faso-space-3);
      border-top: 1px solid var(--faso-border);
      margin-top: var(--faso-space-3);
    }
    .summary div { display: flex; flex-direction: column; gap: 2px; }
    .summary span { color: var(--faso-text-muted); font-size: var(--faso-text-xs); }
    .summary strong {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      color: var(--faso-text);
      font-size: var(--faso-text-base);
    }
    .summary strong mat-icon { font-size: 18px; width: 18px; height: 18px; }
    .summary .positive strong { color: var(--faso-success); }
    .summary .negative strong { color: var(--faso-danger); }
  `],
})
export class GrowthChartComponent {
  /** Actual weight observations, ordered by age */
  @Input() points: GrowthPoint[] = [
    { age: 0,  weight: 45 },
    { age: 7,  weight: 185 },
    { age: 14, weight: 490 },
    { age: 21, weight: 930 },
    { age: 28, weight: 1480 },
    { age: 35, weight: 2080 },
  ];
  @Input() lotLabel = 'L-2026-041';
  @Input() reference: GrowthPoint[] = FAO_REFERENCE;

  readonly width = 600;
  readonly height = 300;
  readonly padL = 50;
  readonly padR = 20;
  readonly padT = 20;
  readonly padB = 30;

  readonly xMax = computed(() => Math.max(
    ...this.reference.map(p => p.age),
    ...this.points.map(p => p.age),
    49,
  ));
  readonly yMax = computed(() => {
    const max = Math.max(
      ...this.reference.map(p => p.weight),
      ...this.points.map(p => p.weight),
      1000,
    );
    return Math.ceil(max / 500) * 500;
  });

  private sx(age: number): number {
    return this.padL + (age / this.xMax()) * (this.width - this.padL - this.padR);
  }
  private sy(w: number): number {
    return this.height - this.padB - (w / this.yMax()) * (this.height - this.padT - this.padB);
  }

  readonly scaledPoints = computed(() =>
    this.points.map(p => ({ age: p.age, x: this.sx(p.age), y: this.sy(p.weight) })),
  );

  readonly linePath = () => this.toPath(this.points);
  readonly refPath  = () => this.toPath(this.reference);

  readonly areaPath = () => {
    if (!this.points.length) return '';
    const first = this.points[0]!;
    const last = this.points[this.points.length - 1]!;
    const baseline = this.height - this.padB;
    const pts = this.points.map(p => `L${this.sx(p.age)},${this.sy(p.weight)}`).join(' ');
    return `M${this.sx(first.age)},${baseline} L${this.sx(first.age)},${this.sy(first.weight)} ${pts} L${this.sx(last.age)},${baseline} Z`;
  };

  readonly yGrid = computed(() => {
    const max = this.yMax();
    const step = max / 4;
    const out: { value: number; pos: number }[] = [];
    for (let i = 0; i <= 4; i++) {
      const v = Math.round(step * i);
      out.push({ value: v, pos: this.sy(v) });
    }
    return out;
  });

  readonly xTicks = computed(() => {
    const max = this.xMax();
    const step = Math.max(7, Math.round(max / 7 / 7) * 7);
    const out: { value: number; pos: number }[] = [];
    for (let v = 0; v <= max; v += step) out.push({ value: v, pos: this.sx(v) });
    return out;
  });

  readonly summary = computed(() => {
    if (!this.points.length) return null;
    const last = this.points[this.points.length - 1]!;
    const ref = this.interpolateRef(last.age);
    const delta = last.weight - ref;
    const pct = ref ? (delta / ref) * 100 : 0;
    return {
      lastWeight: last.weight,
      lastAge: last.age,
      reference: Math.round(ref),
      delta,
      deltaPercent: pct,
    };
  });

  private toPath(pts: GrowthPoint[]): string {
    if (!pts.length) return '';
    return pts
      .map((p, i) => `${i === 0 ? 'M' : 'L'}${this.sx(p.age)},${this.sy(p.weight)}`)
      .join(' ');
  }

  private interpolateRef(age: number): number {
    if (!this.reference.length) return 0;
    for (let i = 0; i < this.reference.length - 1; i++) {
      const a = this.reference[i]!;
      const b = this.reference[i + 1]!;
      if (age >= a.age && age <= b.age) {
        const t = (age - a.age) / (b.age - a.age);
        return a.weight + t * (b.weight - a.weight);
      }
    }
    return this.reference[this.reference.length - 1]!.weight;
  }
}
