// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { KpiCardComponent } from '@shared/components/kpi-card/kpi-card.component';

@Component({
  selector: 'app-admin-kpis',
  standalone: true,
  imports: [CommonModule, DecimalPipe, MatIconModule, SectionHeaderComponent, KpiCardComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <div class="container">
        <header>
          <h1>Vue d'ensemble</h1>
          <p>Plateforme Poulets BF · 7 derniers jours</p>
        </header>

        <div class="kpis">
          <app-kpi-card icon="person"        label="Éleveurs actifs"   value="247"    sublabel="dont 32 nouveaux"        [trend]="12.4" direction="up" />
          <app-kpi-card icon="shopping_bag"  label="Clients actifs"    value="1 412"  sublabel="unique visits"           [trend]="7.8"  direction="up" />
          <app-kpi-card icon="storefront"    label="Annonces publiées" value="186"    sublabel="dont 89 halal certifiées"[trend]="3.2"  direction="up" />
          <app-kpi-card icon="receipt_long"  label="Commandes"         value="72"     sublabel="taux de conversion 5.1%" [trend]="-1.4" direction="down" />
          <app-kpi-card icon="payments"      label="GMV"               value="3.4"    unit="M FCFA" sublabel="incl. TVA" [trend]="15.6" direction="up" />
          <app-kpi-card icon="local_shipping" label="Livraisons réussies" value="94"  unit="%"      sublabel="SLA 95%"   [trend]="0.2"  direction="flat" />
        </div>

        <app-section-header title="Top éleveurs" kicker="Par chiffre d'affaires" />
        <div class="table-card">
          <table>
            <thead>
              <tr>
                <th scope="col">Rang</th>
                <th scope="col">Éleveur</th>
                <th scope="col">Région</th>
                <th scope="col">Commandes</th>
                <th scope="col">CA (FCFA)</th>
                <th scope="col">Note</th>
              </tr>
            </thead>
            <tbody>
              @for (row of topBreeders; track row.id) {
                <tr>
                  <td><span class="rank">#{{ row.rank }}</span></td>
                  <td><strong>{{ row.name }}</strong></td>
                  <td>{{ row.region }}</td>
                  <td>{{ row.orders }}</td>
                  <td><strong>{{ row.revenue | number:'1.0-0' }}</strong></td>
                  <td>
                    <span class="rating">
                      <mat-icon>star</mat-icon>
                      {{ row.rating | number:'1.1-1' }}
                    </span>
                  </td>
                </tr>
              }
            </tbody>
          </table>
        </div>

        <app-section-header title="Santé plateforme" kicker="Infrastructure" />
        <div class="health">
          <div class="health-card ok">
            <mat-icon>check_circle</mat-icon>
            <div>
              <strong>ARMAGEDDON gateway</strong>
              <span>p99 latency 82 ms · 99.98% uptime</span>
            </div>
          </div>
          <div class="health-card ok">
            <mat-icon>check_circle</mat-icon>
            <div>
              <strong>KAYA (cache + state)</strong>
              <span>Réplication 3 nodes · 99.99% uptime</span>
            </div>
          </div>
          <div class="health-card warn">
            <mat-icon>warning</mat-icon>
            <div>
              <strong>ORY Kratos (auth)</strong>
              <span>p95 login 420 ms · à surveiller</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .container {
      max-width: 1400px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-12);
    }
    header { margin-bottom: var(--faso-space-6); }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .kpis {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: var(--faso-space-4);
      margin-bottom: var(--faso-space-10);
    }

    .table-card {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      overflow: auto;
      margin-bottom: var(--faso-space-10);
    }
    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--faso-text-sm);
    }
    th {
      background: var(--faso-surface-alt);
      padding: 10px 16px;
      text-align: left;
      color: var(--faso-text-muted);
      text-transform: uppercase;
      font-size: var(--faso-text-xs);
      letter-spacing: 0.04em;
      font-weight: var(--faso-weight-semibold);
    }
    td {
      padding: 12px 16px;
      border-top: 1px solid var(--faso-border);
    }
    .rank {
      display: inline-flex;
      width: 28px;
      height: 28px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      font-weight: var(--faso-weight-bold);
      border-radius: 50%;
      align-items: center;
      justify-content: center;
    }
    .rating {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      color: var(--faso-accent-700);
      font-weight: var(--faso-weight-semibold);
    }
    .rating mat-icon { font-size: 16px; width: 16px; height: 16px; color: var(--faso-accent-500); }

    .health {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
      gap: var(--faso-space-3);
    }
    .health-card {
      display: flex;
      align-items: center;
      gap: var(--faso-space-3);
      padding: var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-lg);
    }
    .health-card.ok { border-color: var(--faso-success); }
    .health-card.ok mat-icon { color: var(--faso-success); }
    .health-card.warn { border-color: var(--faso-warning); }
    .health-card.warn mat-icon { color: var(--faso-warning); }
    .health-card strong { display: block; }
    .health-card span {
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }
  `],
})
export class AdminKpisComponent {
  readonly topBreeders = [
    { id: 1, rank: 1, name: 'Oumar Traoré (Coopérative)', region: 'Centre-Ouest', orders: 42, revenue: 2150000, rating: 4.7 },
    { id: 2, rank: 2, name: 'Awa Sankara',                region: 'Hauts-Bassins', orders: 38, revenue: 1820000, rating: 4.9 },
    { id: 3, rank: 3, name: 'Kassim Ouédraogo',           region: 'Centre',       orders: 31, revenue: 1410000, rating: 4.8 },
    { id: 4, rank: 4, name: 'Fatim Compaoré',             region: 'Nord',         orders: 22, revenue: 980000,  rating: 4.5 },
    { id: 5, rank: 5, name: 'Issouf Bandé',               region: 'Sahel',        orders: 19, revenue: 840000,  rating: 4.4 },
  ];
}
