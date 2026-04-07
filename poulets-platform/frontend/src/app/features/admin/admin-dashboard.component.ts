import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe, DecimalPipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatTableModule } from '@angular/material/table';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';
import { FcfaCurrencyPipe } from '@shared/pipes/currency.pipe';

interface AdminKpi {
  label: string;
  value: string;
  icon: string;
  color: string;
  trend?: string;
  link: string;
}

interface MonthlyBar {
  month: string;
  value: number;
  percentage: number;
}

interface RecentTransaction {
  id: string;
  date: string;
  from: string;
  to: string;
  amount: number;
  status: string;
}

@Component({
  selector: 'app-admin-dashboard',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatTableModule,
    TranslateModule,
    StatusBadgeComponent,
    FcfaCurrencyPipe,
    DatePipe,
    DecimalPipe,
  ],
  template: `
    <div class="admin-container">
      <div class="page-header">
        <h1>{{ 'admin.dashboard.title' | translate }}</h1>
        <div class="header-links">
          <a mat-stroked-button routerLink="users">
            <mat-icon>people</mat-icon>
            {{ 'admin.dashboard.users' | translate }}
          </a>
          <a mat-stroked-button routerLink="transactions">
            <mat-icon>receipt</mat-icon>
            {{ 'admin.dashboard.transactions' | translate }}
          </a>
          <a mat-stroked-button routerLink="stats">
            <mat-icon>analytics</mat-icon>
            {{ 'admin.dashboard.stats' | translate }}
          </a>
        </div>
      </div>

      <!-- KPI Cards -->
      <div class="kpi-grid">
        @for (kpi of kpis(); track kpi.label) {
          <mat-card class="kpi-card" [style.border-left-color]="kpi.color">
            <a [routerLink]="kpi.link" class="kpi-link">
              <mat-card-content>
                <div class="kpi-content">
                  <div class="kpi-info">
                    <span class="kpi-label">{{ kpi.label | translate }}</span>
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
            </a>
          </mat-card>
        }
      </div>

      <!-- Monthly Transactions Chart -->
      <mat-card class="chart-card">
        <mat-card-header>
          <mat-card-title>{{ 'admin.dashboard.monthly_transactions' | translate }}</mat-card-title>
        </mat-card-header>
        <mat-card-content>
          <div class="bar-chart">
            @for (bar of monthlyData(); track bar.month) {
              <div class="bar-item">
                <div class="bar-wrapper">
                  <div class="bar"
                       [style.height.%]="bar.percentage"
                       [style.background-color]="'#2e7d32'">
                  </div>
                </div>
                <span class="bar-label">{{ bar.month }}</span>
                <span class="bar-value">{{ bar.value }}</span>
              </div>
            }
          </div>
        </mat-card-content>
      </mat-card>

      <!-- Recent Transactions Table -->
      <mat-card>
        <mat-card-header>
          <mat-card-title>{{ 'admin.dashboard.recent_transactions' | translate }}</mat-card-title>
          <span class="spacer"></span>
          <a mat-button color="primary" routerLink="transactions">
            {{ 'common.view_all' | translate }}
          </a>
        </mat-card-header>
        <mat-card-content>
          <table mat-table [dataSource]="recentTransactions()" class="full-width-table">
            <ng-container matColumnDef="date">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.date' | translate }}</th>
              <td mat-cell *matCellDef="let t">{{ t.date | date:'dd/MM/yyyy' }}</td>
            </ng-container>
            <ng-container matColumnDef="from">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.from' | translate }}</th>
              <td mat-cell *matCellDef="let t">{{ t.from }}</td>
            </ng-container>
            <ng-container matColumnDef="to">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.to' | translate }}</th>
              <td mat-cell *matCellDef="let t">{{ t.to }}</td>
            </ng-container>
            <ng-container matColumnDef="amount">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.amount' | translate }}</th>
              <td mat-cell *matCellDef="let t" class="amount-cell">{{ t.amount | fcfa }}</td>
            </ng-container>
            <ng-container matColumnDef="status">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.status' | translate }}</th>
              <td mat-cell *matCellDef="let t">
                <app-status-badge [status]="t.status"></app-status-badge>
              </td>
            </ng-container>
            <tr mat-header-row *matHeaderRowDef="txColumns"></tr>
            <tr mat-row *matRowDef="let row; columns: txColumns;"></tr>
          </table>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .admin-container {
      padding: 24px;
      max-width: 1200px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 24px;
      flex-wrap: wrap;
      gap: 12px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .header-links { display: flex; gap: 8px; }

    .kpi-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .kpi-card {
      border-left: 4px solid;
    }

    .kpi-link {
      text-decoration: none;
      color: inherit;
    }

    .kpi-content {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    .kpi-info {
      display: flex;
      flex-direction: column;
    }

    .kpi-label { font-size: 0.85rem; color: #666; margin-bottom: 4px; }
    .kpi-value { font-size: 1.6rem; font-weight: 700; color: #333; }
    .kpi-trend {
      font-size: 0.8rem;
      color: #e53935;
      margin-top: 2px;

      &.positive { color: #43a047; }
    }

    .kpi-icon {
      font-size: 40px;
      width: 40px;
      height: 40px;
      opacity: 0.7;
    }

    .chart-card {
      margin-bottom: 24px;

      mat-card-header { margin-bottom: 16px; }
    }

    .bar-chart {
      display: flex;
      align-items: flex-end;
      justify-content: space-around;
      height: 220px;
      padding: 0 8px;
    }

    .bar-item {
      display: flex;
      flex-direction: column;
      align-items: center;
      flex: 1;
      max-width: 50px;
    }

    .bar-wrapper {
      width: 32px;
      height: 180px;
      display: flex;
      align-items: flex-end;
    }

    .bar {
      width: 100%;
      border-radius: 4px 4px 0 0;
      transition: height 0.6s ease;
      min-height: 4px;
    }

    .bar-label { font-size: 0.75rem; color: #666; margin-top: 8px; }
    .bar-value { font-size: 0.65rem; color: #999; }

    .full-width-table { width: 100%; }
    .amount-cell { font-weight: 500; }

    mat-card-header {
      display: flex;
      align-items: center;

      .spacer { flex: 1; }
    }
  `],
})
export class AdminDashboardComponent implements OnInit {
  readonly kpis = signal<AdminKpi[]>([]);
  readonly monthlyData = signal<MonthlyBar[]>([]);
  readonly recentTransactions = signal<RecentTransaction[]>([]);
  readonly txColumns = ['date', 'from', 'to', 'amount', 'status'];

  ngOnInit(): void {
    this.loadKpis();
    this.loadChart();
    this.loadTransactions();
  }

  private loadKpis(): void {
    this.kpis.set([
      { label: 'admin.kpi.total_users', value: '342', icon: 'people', color: '#2196f3', trend: '+28 ce mois', link: 'users' },
      { label: 'admin.kpi.monthly_transactions', value: '156', icon: 'receipt_long', color: '#4caf50', trend: '+12%', link: 'transactions' },
      { label: 'admin.kpi.volume_fcfa', value: '8 450 000', icon: 'payments', color: '#ff9800', trend: '+18%', link: 'stats' },
      { label: 'admin.kpi.matching_rate', value: '78%', icon: 'link', color: '#9c27b0', trend: '+5%', link: 'stats' },
    ]);
  }

  private loadChart(): void {
    const data = [
      { month: 'Nov', value: 98 },
      { month: 'Dec', value: 120 },
      { month: 'Jan', value: 105 },
      { month: 'Fev', value: 134 },
      { month: 'Mar', value: 145 },
      { month: 'Avr', value: 156 },
    ];
    const max = Math.max(...data.map(d => d.value));
    this.monthlyData.set(data.map(d => ({
      ...d,
      percentage: (d.value / max) * 100,
    })));
  }

  private loadTransactions(): void {
    this.recentTransactions.set([
      { id: 't1', date: '2026-04-07', from: 'Restaurant Le Sahel', to: 'Ferme Ouedraogo', amount: 175000, status: 'CONFIRMEE' },
      { id: 't2', date: '2026-04-06', from: 'Mme Traore', to: 'Ferme Kabore', amount: 80000, status: 'EN_ATTENTE' },
      { id: 't3', date: '2026-04-05', from: 'Hotel Splendide', to: 'Groupement Koudougou', amount: 300000, status: 'LIVREE' },
      { id: 't4', date: '2026-04-04', from: 'M. Kabore', to: 'Ferme Ouedraogo', amount: 35000, status: 'LIVREE' },
      { id: 't5', date: '2026-04-03', from: 'Brasserie du Centre', to: 'Ferme Ouedraogo', amount: 150000, status: 'ANNULEE' },
    ]);
  }
}
