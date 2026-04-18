// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

import { GrowthChartComponent, GrowthPoint } from './growth-chart.component';

@Component({
  selector: 'app-growth-chart-page',
  standalone: true,
  imports: [CommonModule, RouterLink, MatIconModule, MatButtonModule, GrowthChartComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <div class="container">
        <a mat-button routerLink=".." class="back">
          <mat-icon>arrow_back</mat-icon> Retour au lot
        </a>

        <app-growth-chart [lotLabel]="lotId()" [points]="points()" />
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .container {
      max-width: 900px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-12);
    }
    .back { color: var(--faso-text-muted); margin-bottom: var(--faso-space-4); margin-left: calc(var(--faso-space-4) * -1); }
  `],
})
export class GrowthChartPageComponent {
  private readonly route = inject(ActivatedRoute);

  readonly lotId = signal<string>(this.route.snapshot.paramMap.get('lotId') ?? 'L-2026-041');

  // Stub data — replace with real service call when GraphQL ready.
  readonly points = signal<GrowthPoint[]>([
    { age: 0,  weight: 45 },
    { age: 7,  weight: 185 },
    { age: 14, weight: 490 },
    { age: 21, weight: 930 },
    { age: 28, weight: 1480 },
    { age: 35, weight: 2080 },
  ]);
}
