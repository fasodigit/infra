import { Component, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { TranslateModule } from '@ngx-translate/core';

@Component({
  selector: 'app-admin-dashboard',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    TranslateModule,
  ],
  template: `
    <div class="dashboard-container">
      <div class="dashboard-header">
        <h1>{{ 'admin.global_view' | translate }}</h1>
      </div>

      <div class="kpi-grid">
        <mat-card class="kpi-card" style="border-left-color: #2196f3">
          <mat-card-content>
            <div class="kpi-content">
              <div class="kpi-info">
                <span class="kpi-label">{{ 'menu.users' | translate }}</span>
                <span class="kpi-value">248</span>
              </div>
              <mat-icon class="kpi-icon" style="color: #2196f3">people</mat-icon>
            </div>
          </mat-card-content>
        </mat-card>

        <mat-card class="kpi-card" style="border-left-color: #4caf50">
          <mat-card-content>
            <div class="kpi-content">
              <div class="kpi-info">
                <span class="kpi-label">{{ 'menu.transactions' | translate }}</span>
                <span class="kpi-value">1 024</span>
              </div>
              <mat-icon class="kpi-icon" style="color: #4caf50">receipt_long</mat-icon>
            </div>
          </mat-card-content>
        </mat-card>

        <mat-card class="kpi-card" style="border-left-color: #ff9800">
          <mat-card-content>
            <div class="kpi-content">
              <div class="kpi-info">
                <span class="kpi-label">{{ 'menu.statistics' | translate }}</span>
                <span class="kpi-value">+15%</span>
              </div>
              <mat-icon class="kpi-icon" style="color: #ff9800">bar_chart</mat-icon>
            </div>
          </mat-card-content>
        </mat-card>

        <mat-card class="kpi-card" style="border-left-color: #9c27b0">
          <mat-card-content>
            <div class="kpi-content">
              <div class="kpi-info">
                <span class="kpi-label">{{ 'menu.geographic_zones' | translate }}</span>
                <span class="kpi-value">13</span>
              </div>
              <mat-icon class="kpi-icon" style="color: #9c27b0">map</mat-icon>
            </div>
          </mat-card-content>
        </mat-card>
      </div>
    </div>
  `,
  styles: [`
    .dashboard-container { padding: 24px; max-width: 1400px; margin: 0 auto; }
    .dashboard-header { margin-bottom: 24px; }
    .dashboard-header h1 { margin: 0; font-size: 1.8rem; color: var(--faso-primary-dark, #1b5e20); }
    .kpi-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 16px; }
    .kpi-card { border-left: 4px solid; }
    .kpi-content { display: flex; justify-content: space-between; align-items: center; }
    .kpi-info { display: flex; flex-direction: column; }
    .kpi-label { font-size: 0.85rem; color: #666; margin-bottom: 4px; }
    .kpi-value { font-size: 1.5rem; font-weight: 700; color: #333; }
    .kpi-icon { font-size: 40px; width: 40px; height: 40px; opacity: 0.7; }
  `],
})
export class AdminDashboardComponent {}
