import { Component, OnInit, inject, signal, computed } from '@angular/core';
import { CommonModule, DecimalPipe, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatTableModule } from '@angular/material/table';
import { MatChipsModule } from '@angular/material/chips';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatBadgeModule } from '@angular/material/badge';
import { TranslateModule } from '@ngx-translate/core';
import { AuthService } from '../../core/services/auth.service';

interface KpiCard {
  label: string;
  value: string | number;
  icon: string;
  color: string;
  trend?: string;
}

interface RecentOrder {
  id: string;
  client: string;
  quantite: number;
  dateLivraison: string;
  statut: string;
}

interface Alert {
  type: 'warning' | 'error' | 'info';
  icon: string;
  message: string;
}

@Component({
  selector: 'app-eleveur-dashboard',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatTableModule,
    MatChipsModule,
    MatProgressSpinnerModule,
    MatBadgeModule,
    TranslateModule,
    DecimalPipe,
    DatePipe,
  ],
  template: `
    <div class="dashboard-container">
      <div class="dashboard-header">
        <h1>{{ 'dashboard.title' | translate }}</h1>
        <p class="welcome-text">{{ 'dashboard.welcome' | translate:{ name: userName() } }}</p>
      </div>

      <!-- KPI Cards -->
      <div class="kpi-grid">
        @for (kpi of kpis(); track kpi.label) {
          <mat-card class="kpi-card" [style.border-left-color]="kpi.color">
            <mat-card-content>
              <div class="kpi-content">
                <div class="kpi-info">
                  <span class="kpi-label">{{ kpi.label }}</span>
                  <span class="kpi-value">{{ kpi.value }}</span>
                  @if (kpi.trend) {
                    <span class="kpi-trend" [class.positive]="kpi.trend.startsWith('+')">
                      {{ kpi.trend }}
                    </span>
                  }
                </div>
                <mat-icon class="kpi-icon" [style.color]="kpi.color">{{ kpi.icon }}</mat-icon>
              </div>
            </mat-card-content>
          </mat-card>
        }
      </div>

      <!-- Charts Row -->
      <div class="charts-row">
        <!-- Revenue Chart (Bar) -->
        <mat-card class="chart-card">
          <mat-card-header>
            <mat-card-title>Revenus sur 6 mois (FCFA)</mat-card-title>
          </mat-card-header>
          <mat-card-content>
            <div class="bar-chart">
              @for (bar of revenueData(); track bar.month) {
                <div class="bar-item">
                  <div class="bar-wrapper">
                    <div
                      class="bar"
                      [style.height.%]="bar.percentage"
                      [style.background-color]="'#4caf50'"
                    ></div>
                  </div>
                  <span class="bar-label">{{ bar.month }}</span>
                  <span class="bar-value">{{ bar.value | number:'1.0-0' }}</span>
                </div>
              }
            </div>
          </mat-card-content>
        </mat-card>

        <!-- Weight Chart (Line) -->
        <mat-card class="chart-card">
          <mat-card-header>
            <mat-card-title>Poids moyen par lot vs objectif (kg)</mat-card-title>
          </mat-card-header>
          <mat-card-content>
            <div class="line-chart-container">
              <svg viewBox="0 0 400 200" class="line-chart">
                <!-- Grid lines -->
                @for (y of [0, 50, 100, 150, 200]; track y) {
                  <line [attr.x1]="40" [attr.y1]="y" [attr.x2]="390" [attr.y2]="y"
                        stroke="#e0e0e0" stroke-width="0.5"/>
                }
                <!-- Actual weight line -->
                <polyline
                  [attr.points]="weightActualPoints()"
                  fill="none" stroke="#4caf50" stroke-width="2.5"/>
                <!-- Target weight line -->
                <polyline
                  [attr.points]="weightTargetPoints()"
                  fill="none" stroke="#ff9800" stroke-width="2" stroke-dasharray="6,3"/>
                <!-- Legend -->
                <rect x="50" y="10" width="12" height="3" fill="#4caf50"/>
                <text x="66" y="13" font-size="9" fill="#666">Poids moyen</text>
                <rect x="160" y="10" width="12" height="3" fill="#ff9800"/>
                <text x="176" y="13" font-size="9" fill="#666">Objectif</text>
                <!-- X axis labels -->
                @for (label of weightLabels(); track label.x) {
                  <text [attr.x]="label.x" y="198" font-size="8" fill="#666" text-anchor="middle">
                    {{ label.text }}
                  </text>
                }
              </svg>
            </div>
          </mat-card-content>
        </mat-card>
      </div>

      <!-- Recent Orders Table -->
      <mat-card class="table-card">
        <mat-card-header>
          <mat-card-title>{{ 'dashboard.recent_orders' | translate }}</mat-card-title>
          <span class="spacer"></span>
          <a mat-button color="primary" routerLink="/orders">
            {{ 'common.view_all' | translate }}
          </a>
        </mat-card-header>
        <mat-card-content>
          <table mat-table [dataSource]="recentOrders()" class="full-width-table">
            <ng-container matColumnDef="client">
              <th mat-header-cell *matHeaderCellDef>Client</th>
              <td mat-cell *matCellDef="let order">{{ order.client }}</td>
            </ng-container>
            <ng-container matColumnDef="quantite">
              <th mat-header-cell *matHeaderCellDef>{{ 'contracts.quantity' | translate }}</th>
              <td mat-cell *matCellDef="let order">{{ order.quantite }}</td>
            </ng-container>
            <ng-container matColumnDef="dateLivraison">
              <th mat-header-cell *matHeaderCellDef>Date livraison</th>
              <td mat-cell *matCellDef="let order">{{ order.dateLivraison | date:'dd/MM/yyyy' }}</td>
            </ng-container>
            <ng-container matColumnDef="statut">
              <th mat-header-cell *matHeaderCellDef>{{ 'orders.status' | translate }}</th>
              <td mat-cell *matCellDef="let order">
                <mat-chip [class]="'status-' + order.statut.toLowerCase()">
                  {{ order.statut }}
                </mat-chip>
              </td>
            </ng-container>
            <tr mat-header-row *matHeaderRowDef="orderColumns"></tr>
            <tr mat-row *matRowDef="let row; columns: orderColumns;"></tr>
          </table>
        </mat-card-content>
      </mat-card>

      <!-- Alerts Section -->
      <mat-card class="alerts-card">
        <mat-card-header>
          <mat-card-title>
            <mat-icon>notifications</mat-icon>
            Alertes
          </mat-card-title>
        </mat-card-header>
        <mat-card-content>
          @for (alert of alerts(); track alert.message) {
            <div class="alert-item" [class]="'alert-' + alert.type">
              <mat-icon>{{ alert.icon }}</mat-icon>
              <span>{{ alert.message }}</span>
            </div>
          } @empty {
            <p class="no-alerts">Aucune alerte</p>
          }
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .dashboard-container {
      padding: 24px;
      max-width: 1400px;
      margin: 0 auto;
    }

    .dashboard-header {
      margin-bottom: 24px;

      h1 {
        margin: 0;
        font-size: 1.8rem;
        color: var(--faso-primary-dark, #1b5e20);
      }

      .welcome-text {
        color: #666;
        margin: 4px 0 0;
      }
    }

    .kpi-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .kpi-card {
      border-left: 4px solid;

      .kpi-content {
        display: flex;
        justify-content: space-between;
        align-items: center;
      }

      .kpi-info {
        display: flex;
        flex-direction: column;
      }

      .kpi-label {
        font-size: 0.85rem;
        color: #666;
        margin-bottom: 4px;
      }

      .kpi-value {
        font-size: 1.6rem;
        font-weight: 700;
        color: #333;
      }

      .kpi-trend {
        font-size: 0.8rem;
        color: #e53935;
        margin-top: 2px;

        &.positive {
          color: #43a047;
        }
      }

      .kpi-icon {
        font-size: 40px;
        width: 40px;
        height: 40px;
        opacity: 0.7;
      }
    }

    .charts-row {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .chart-card {
      mat-card-header {
        margin-bottom: 16px;
      }
    }

    .bar-chart {
      display: flex;
      align-items: flex-end;
      justify-content: space-around;
      height: 200px;
      padding: 0 8px;
    }

    .bar-item {
      display: flex;
      flex-direction: column;
      align-items: center;
      flex: 1;
      max-width: 60px;
    }

    .bar-wrapper {
      width: 32px;
      height: 160px;
      display: flex;
      align-items: flex-end;
    }

    .bar {
      width: 100%;
      border-radius: 4px 4px 0 0;
      transition: height 0.6s ease;
      min-height: 4px;
    }

    .bar-label {
      font-size: 0.75rem;
      color: #666;
      margin-top: 8px;
    }

    .bar-value {
      font-size: 0.65rem;
      color: #999;
    }

    .line-chart-container {
      width: 100%;
      max-height: 220px;
    }

    .line-chart {
      width: 100%;
      height: auto;
    }

    .table-card {
      margin-bottom: 24px;

      mat-card-header {
        display: flex;
        align-items: center;

        .spacer {
          flex: 1;
        }
      }
    }

    .full-width-table {
      width: 100%;
    }

    .status-confirmee { background-color: #e8f5e9 !important; color: #2e7d32 !important; }
    .status-en_attente { background-color: #fff3e0 !important; color: #ef6c00 !important; }
    .status-en_livraison { background-color: #e3f2fd !important; color: #1565c0 !important; }
    .status-livree { background-color: #f3e5f5 !important; color: #7b1fa2 !important; }

    .alerts-card {
      mat-card-header mat-card-title {
        display: flex;
        align-items: center;
        gap: 8px;
      }
    }

    .alert-item {
      display: flex;
      align-items: center;
      gap: 12px;
      padding: 12px 16px;
      border-radius: 8px;
      margin-bottom: 8px;
      font-size: 0.9rem;

      &.alert-warning {
        background: #fff3e0;
        color: #e65100;
      }

      &.alert-error {
        background: #fce4ec;
        color: #c62828;
      }

      &.alert-info {
        background: #e3f2fd;
        color: #1565c0;
      }
    }

    .no-alerts {
      text-align: center;
      color: #999;
      padding: 24px;
    }
  `],
})
export class EleveurDashboardComponent implements OnInit {
  private readonly auth = inject(AuthService);

  readonly userName = computed(() => this.auth.currentUser()?.nom ?? 'Eleveur');

  readonly kpis = signal<KpiCard[]>([]);
  readonly revenueData = signal<{ month: string; value: number; percentage: number }[]>([]);
  readonly weightLabels = signal<{ x: number; text: string }[]>([]);
  readonly recentOrders = signal<RecentOrder[]>([]);
  readonly alerts = signal<Alert[]>([]);

  readonly orderColumns = ['client', 'quantite', 'dateLivraison', 'statut'];

  readonly weightActualPoints = signal('');
  readonly weightTargetPoints = signal('');

  ngOnInit(): void {
    this.loadKpis();
    this.loadRevenueChart();
    this.loadWeightChart();
    this.loadRecentOrders();
    this.loadAlerts();
  }

  private loadKpis(): void {
    this.kpis.set([
      { label: 'Lots actifs', value: 5, icon: 'inventory_2', color: '#4caf50', trend: '+2 ce mois' },
      { label: 'Poulets total', value: '1 240', icon: 'egg_alt', color: '#2196f3' },
      { label: 'Commandes en cours', value: 8, icon: 'shopping_cart', color: '#ff9800', trend: '+3 cette semaine' },
      { label: 'Revenu du mois (FCFA)', value: '485 000', icon: 'payments', color: '#9c27b0', trend: '+12%' },
      { label: 'Taux de ponctualite', value: '94%', icon: 'schedule', color: '#00bcd4' },
    ]);
  }

  private loadRevenueChart(): void {
    const data = [
      { month: 'Nov', value: 320000 },
      { month: 'Dec', value: 410000 },
      { month: 'Jan', value: 380000 },
      { month: 'Fev', value: 450000 },
      { month: 'Mar', value: 520000 },
      { month: 'Avr', value: 485000 },
    ];
    const max = Math.max(...data.map(d => d.value));
    this.revenueData.set(data.map(d => ({
      ...d,
      percentage: (d.value / max) * 100,
    })));
  }

  private loadWeightChart(): void {
    // Simulated weight data over 8 weeks
    const actual = [0.15, 0.35, 0.65, 1.0, 1.35, 1.65, 1.9, 2.1];
    const target = [0.15, 0.40, 0.70, 1.05, 1.40, 1.75, 2.0, 2.2];
    const maxW = 2.5;
    const chartW = 350;
    const chartH = 170;
    const offsetX = 50;
    const offsetY = 20;

    const toPoint = (values: number[]) =>
      values.map((v, i) => {
        const x = offsetX + (i / (values.length - 1)) * chartW;
        const y = offsetY + chartH - (v / maxW) * chartH;
        return `${x},${y}`;
      }).join(' ');

    this.weightActualPoints.set(toPoint(actual));
    this.weightTargetPoints.set(toPoint(target));

    this.weightLabels.set(
      ['S1', 'S2', 'S3', 'S4', 'S5', 'S6', 'S7', 'S8'].map((text, i) => ({
        x: offsetX + (i / 7) * chartW,
        text,
      }))
    );
  }

  private loadRecentOrders(): void {
    this.recentOrders.set([
      { id: 'CMD-001', client: 'Restaurant Le Sahel', quantite: 50, dateLivraison: '2026-04-10', statut: 'Confirmee' },
      { id: 'CMD-002', client: 'Mme Ouedraogo', quantite: 15, dateLivraison: '2026-04-08', statut: 'En_attente' },
      { id: 'CMD-003', client: 'Hotel Splendide', quantite: 100, dateLivraison: '2026-04-12', statut: 'En_livraison' },
      { id: 'CMD-004', client: 'M. Kabore', quantite: 10, dateLivraison: '2026-04-06', statut: 'Livree' },
      { id: 'CMD-005', client: 'Brasserie du Centre', quantite: 30, dateLivraison: '2026-04-09', statut: 'Confirmee' },
    ]);
  }

  private loadAlerts(): void {
    this.alerts.set([
      { type: 'warning', icon: 'trending_down', message: 'Lot #3 (Brahma) : retard de croissance de 15% par rapport a l\'objectif' },
      { type: 'error', icon: 'medical_services', message: 'Certificat veterinaire du Lot #1 expire dans 5 jours' },
      { type: 'info', icon: 'assignment', message: '3 commandes a preparer pour cette semaine' },
    ]);
  }
}
