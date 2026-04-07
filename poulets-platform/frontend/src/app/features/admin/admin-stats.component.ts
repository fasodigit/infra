import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { TranslateModule } from '@ngx-translate/core';
import { FcfaCurrencyPipe } from '@shared/pipes/currency.pipe';

interface StatCard {
  label: string;
  value: string;
  icon: string;
  color: string;
}

@Component({
  selector: 'app-admin-stats',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    TranslateModule,
    FcfaCurrencyPipe,
    DecimalPipe,
  ],
  template: `
    <div class="stats-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'admin.stats.title' | translate }}</h1>
      </div>

      <!-- Platform Stats -->
      <div class="stats-grid">
        @for (stat of platformStats(); track stat.label) {
          <mat-card class="stat-card" [style.border-top-color]="stat.color">
            <mat-card-content>
              <mat-icon [style.color]="stat.color">{{ stat.icon }}</mat-icon>
              <span class="stat-value">{{ stat.value }}</span>
              <span class="stat-label">{{ stat.label | translate }}</span>
            </mat-card-content>
          </mat-card>
        }
      </div>

      <!-- Race Distribution -->
      <mat-card class="chart-card">
        <mat-card-header>
          <mat-card-title>{{ 'admin.stats.race_distribution' | translate }}</mat-card-title>
        </mat-card-header>
        <mat-card-content>
          <div class="distribution-bars">
            @for (race of raceDistribution(); track race.name) {
              <div class="dist-row">
                <span class="dist-label">{{ race.name }}</span>
                <div class="dist-bar-track">
                  <div class="dist-bar-fill"
                       [style.width.%]="race.percentage"
                       [style.background-color]="race.color">
                  </div>
                </div>
                <span class="dist-value">{{ race.percentage | number:'1.0-0' }}%</span>
              </div>
            }
          </div>
        </mat-card-content>
      </mat-card>

      <!-- Regional Stats -->
      <mat-card>
        <mat-card-header>
          <mat-card-title>{{ 'admin.stats.regional' | translate }}</mat-card-title>
        </mat-card-header>
        <mat-card-content>
          <div class="regional-grid">
            @for (region of regionalStats(); track region.name) {
              <div class="region-item">
                <mat-icon>location_on</mat-icon>
                <div class="region-info">
                  <span class="region-name">{{ region.name }}</span>
                  <span class="region-details">
                    {{ region.eleveurs }} {{ 'admin.stats.eleveurs' | translate }},
                    {{ region.clients }} {{ 'admin.stats.clients' | translate }}
                  </span>
                  <span class="region-volume">{{ region.volume | fcfa }}</span>
                </div>
              </div>
            }
          </div>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .stats-container {
      padding: 24px;
      max-width: 1000px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .stat-card {
      border-top: 4px solid;

      mat-card-content {
        display: flex;
        flex-direction: column;
        align-items: center;
        text-align: center;
        gap: 8px;

        mat-icon { font-size: 32px; width: 32px; height: 32px; }
        .stat-value { font-size: 1.8rem; font-weight: 700; }
        .stat-label { font-size: 0.8rem; color: #666; }
      }
    }

    .chart-card { margin-bottom: 24px; }

    .distribution-bars {
      display: flex;
      flex-direction: column;
      gap: 12px;
      padding: 16px 0;
    }

    .dist-row {
      display: flex;
      align-items: center;
      gap: 12px;

      .dist-label { width: 140px; font-size: 0.9rem; }

      .dist-bar-track {
        flex: 1;
        height: 24px;
        background: #f0f0f0;
        border-radius: 12px;
        overflow: hidden;
      }

      .dist-bar-fill {
        height: 100%;
        border-radius: 12px;
        transition: width 0.6s ease;
      }

      .dist-value { width: 40px; text-align: right; font-weight: 500; }
    }

    .regional-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
      gap: 16px;
      padding: 16px 0;
    }

    .region-item {
      display: flex;
      align-items: flex-start;
      gap: 12px;
      padding: 12px;
      border: 1px solid #e0e0e0;
      border-radius: 8px;

      mat-icon { color: var(--faso-primary, #2e7d32); margin-top: 2px; }
    }

    .region-info {
      display: flex;
      flex-direction: column;
      gap: 4px;

      .region-name { font-weight: 600; }
      .region-details { font-size: 0.8rem; color: #666; }
      .region-volume { font-weight: 500; color: var(--faso-primary-dark, #1b5e20); }
    }
  `],
})
export class AdminStatsComponent implements OnInit {
  readonly platformStats = signal<StatCard[]>([]);
  readonly raceDistribution = signal<{ name: string; percentage: number; color: string }[]>([]);
  readonly regionalStats = signal<{ name: string; eleveurs: number; clients: number; volume: number }[]>([]);

  ngOnInit(): void {
    this.loadStats();
  }

  private loadStats(): void {
    this.platformStats.set([
      { label: 'admin.stats.total_eleveurs', value: '185', icon: 'agriculture', color: '#4caf50' },
      { label: 'admin.stats.total_clients', value: '142', icon: 'restaurant', color: '#2196f3' },
      { label: 'admin.stats.total_lots', value: '423', icon: 'inventory_2', color: '#ff9800' },
      { label: 'admin.stats.total_volume', value: '45.2M', icon: 'payments', color: '#9c27b0' },
      { label: 'admin.stats.avg_order', value: '125 000', icon: 'shopping_cart', color: '#00bcd4' },
      { label: 'admin.stats.halal_certs', value: '89', icon: 'verified', color: '#795548' },
    ]);

    this.raceDistribution.set([
      { name: 'Poulet bicyclette', percentage: 42, color: '#4caf50' },
      { name: 'Poulet de chair', percentage: 28, color: '#2196f3' },
      { name: 'Pintade', percentage: 15, color: '#ff9800' },
      { name: 'Dinde', percentage: 8, color: '#9c27b0' },
      { name: 'Autres', percentage: 7, color: '#607d8b' },
    ]);

    this.regionalStats.set([
      { name: 'Ouagadougou', eleveurs: 45, clients: 60, volume: 18500000 },
      { name: 'Bobo-Dioulasso', eleveurs: 35, clients: 30, volume: 10200000 },
      { name: 'Koudougou', eleveurs: 28, clients: 15, volume: 6800000 },
      { name: 'Ouahigouya', eleveurs: 22, clients: 12, volume: 4500000 },
      { name: 'Banfora', eleveurs: 18, clients: 10, volume: 3200000 },
      { name: 'Kaya', eleveurs: 15, clients: 8, volume: 2000000 },
    ]);
  }
}
