// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatCardModule } from '@angular/material/card';
import { MatIconModule } from '@angular/material/icon';

// TODO(FASO-F10): ngx-charts + query poulets-api aggregations
//   - Ajouter dépendance @swimlane/ngx-charts (alternative souveraine à évaluer)
//   - Queries GraphQL côté BFF :
//       vendorAnalytics(period: "30d") {
//         revenue, orderCount, conversionRate, slaCompliance,
//         revenueByDay, ordersByStatus, topProducts
//       }
//   - Agrégations calculées côté poulets-api (Spring Data JPA / Projections)
//     avec cache KAYA (TTL 10 min)
//   - Graphiques : line chart CA/jour, bar chart commandes/statut,
//     gauge conversion + SLA

interface KpiStub {
  readonly key: string;
  readonly label: string;
  readonly icon: string;
  readonly placeholder: string;
}

@Component({
  selector: 'app-analytics',
  standalone: true,
  imports: [CommonModule, MatCardModule, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="stub">
      <h1>Analytique vendeur</h1>
      <p>TODO(FASO-F10): ngx-charts + query poulets-api aggregations</p>

      <div class="kpi-grid">
        @for (kpi of kpis; track kpi.key) {
          <mat-card class="kpi-card" [attr.data-testid]="'kpi-' + kpi.key">
            <mat-card-content>
              <div class="kpi-head">
                <mat-icon>{{ kpi.icon }}</mat-icon>
                <span class="kpi-label">{{ kpi.label }}</span>
              </div>
              <div class="kpi-value">{{ kpi.placeholder }}</div>
            </mat-card-content>
          </mat-card>
        }
      </div>
    </section>
  `,
  styles: [`
    .stub { padding: 24px; max-width: 1080px; margin: 0 auto; }
    .stub h1 { font-size: 1.75rem; margin-bottom: 12px; }
    .stub p { color: #555; margin: 8px 0 24px; }
    .kpi-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 16px;
    }
    .kpi-card { padding: 8px; }
    .kpi-head { display: flex; align-items: center; gap: 8px; color: #666; font-size: 0.9rem; }
    .kpi-label { font-weight: 500; }
    .kpi-value {
      font-size: 1.6rem;
      font-weight: 700;
      margin-top: 8px;
      color: #bbb;
      font-style: italic;
    }
  `],
})
export class AnalyticsComponent {
  readonly kpis: readonly KpiStub[] = [
    { key: 'ca', label: 'Chiffre d\'affaires', icon: 'payments', placeholder: '— FCFA' },
    { key: 'orders', label: 'Commandes', icon: 'shopping_cart', placeholder: '—' },
    { key: 'conversion', label: 'Taux de conversion', icon: 'trending_up', placeholder: '— %' },
    { key: 'sla', label: 'SLA livraison', icon: 'schedule', placeholder: '— %' },
  ];
}
