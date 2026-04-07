import { Component, OnInit, inject, signal, computed, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, FormGroup } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatSelectModule } from '@angular/material/select';
import { MatInputModule } from '@angular/material/input';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatChipsModule } from '@angular/material/chips';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';

import { CalendarService } from '../services/calendar.service';
import { PlanningData, PlanningFilter, SupplyDemandWeek } from '../../../shared/models/calendar.models';
import { CHICKEN_RACES } from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-planning',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    ReactiveFormsModule,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatFormFieldModule,
    MatSelectModule,
    MatInputModule,
    MatProgressSpinnerModule,
    MatChipsModule,
    MatTooltipModule,
    MatDividerModule,
    TranslateModule,
  ],
  template: `
    <div class="planning-page">
      <div class="page-header">
        <div>
          <h1>
            <mat-icon>timeline</mat-icon>
            {{ 'calendar.planning.title' | translate }}
          </h1>
          <p class="subtitle">{{ 'calendar.planning.subtitle' | translate }}</p>
        </div>
        <a mat-stroked-button routerLink="/calendar">
          <mat-icon>calendar_month</mat-icon>
          {{ 'calendar.planning.backToCalendar' | translate }}
        </a>
      </div>

      <!-- Filters -->
      <mat-card class="filter-card">
        <mat-card-content>
          <form [formGroup]="filterForm" (ngSubmit)="applyFilters()" class="filter-form">
            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'calendar.planning.filterRace' | translate }}</mat-label>
              <mat-select formControlName="race">
                <mat-option value="">{{ 'calendar.planning.allRaces' | translate }}</mat-option>
                @for (race of races; track race) {
                  <mat-option [value]="race">{{ race }}</mat-option>
                }
              </mat-select>
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'calendar.planning.filterLocation' | translate }}</mat-label>
              <input matInput formControlName="location">
            </mat-form-field>

            <div class="filter-actions">
              <button mat-raised-button color="primary" type="submit">
                <mat-icon>refresh</mat-icon>
                {{ 'calendar.planning.refresh' | translate }}
              </button>
            </div>
          </form>
        </mat-card-content>
      </mat-card>

      @if (loading()) {
        <div class="loading-container">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else {
        <!-- Summary Cards -->
        <div class="summary-cards">
          <mat-card class="summary-card supply">
            <mat-card-content>
              <mat-icon>egg_alt</mat-icon>
              <div class="summary-info">
                <span class="summary-value">{{ totalSupply() | number }}</span>
                <span class="summary-label">{{ 'calendar.planning.totalSupply' | translate }}</span>
              </div>
            </mat-card-content>
          </mat-card>
          <mat-card class="summary-card demand">
            <mat-card-content>
              <mat-icon>shopping_bag</mat-icon>
              <div class="summary-info">
                <span class="summary-value">{{ totalDemand() | number }}</span>
                <span class="summary-label">{{ 'calendar.planning.totalDemand' | translate }}</span>
              </div>
            </mat-card-content>
          </mat-card>
          <mat-card class="summary-card gap"
            [class.surplus]="totalGap() > 0"
            [class.deficit]="totalGap() < 0">
            <mat-card-content>
              <mat-icon>{{ totalGap() >= 0 ? 'trending_up' : 'trending_down' }}</mat-icon>
              <div class="summary-info">
                <span class="summary-value">{{ totalGap() > 0 ? '+' : '' }}{{ totalGap() | number }}</span>
                <span class="summary-label">{{ 'calendar.planning.gap' | translate }}</span>
              </div>
            </mat-card-content>
          </mat-card>
        </div>

        <!-- Per-Race Timelines -->
        @for (raceData of planningData(); track raceData.race) {
          <mat-card class="race-timeline-card">
            <mat-card-header>
              <mat-icon mat-card-avatar class="race-icon">egg_alt</mat-icon>
              <mat-card-title>{{ raceData.race }}</mat-card-title>
              <mat-card-subtitle>{{ 'calendar.planning.weeklyTimeline' | translate }}</mat-card-subtitle>
            </mat-card-header>
            <mat-card-content>
              <!-- Legend -->
              <div class="chart-legend">
                <span class="legend-item">
                  <span class="legend-dot supply-dot"></span>
                  {{ 'calendar.planning.supply' | translate }}
                </span>
                <span class="legend-item">
                  <span class="legend-dot demand-dot"></span>
                  {{ 'calendar.planning.demand' | translate }}
                </span>
              </div>

              <!-- Bar Chart -->
              <div class="timeline-chart">
                @for (week of raceData.weeks; track week.weekStart) {
                  <div class="week-column"
                    [matTooltip]="week.weekLabel + ': ' + week.supply + ' offre / ' + week.demand + ' demande'">
                    <div class="bars-container">
                      <div class="bar supply-bar"
                        [style.height.%]="getBarHeight(week.supply, raceData.weeks)">
                        <span class="bar-value">{{ week.supply }}</span>
                      </div>
                      <div class="bar demand-bar"
                        [style.height.%]="getBarHeight(week.demand, raceData.weeks)">
                        <span class="bar-value">{{ week.demand }}</span>
                      </div>
                    </div>
                    <div class="week-label">{{ week.weekLabel }}</div>
                    <div class="gap-indicator"
                      [class.surplus]="week.gap > 0"
                      [class.deficit]="week.gap < 0"
                      [class.balanced]="week.gap === 0">
                      @if (week.gap > 0) {
                        <mat-icon>arrow_upward</mat-icon>
                        +{{ week.gap }}
                      } @else if (week.gap < 0) {
                        <mat-icon>arrow_downward</mat-icon>
                        {{ week.gap }}
                      } @else {
                        <mat-icon>remove</mat-icon>
                        0
                      }
                    </div>
                  </div>
                }
              </div>
            </mat-card-content>
          </mat-card>
        }

        @if (planningData().length === 0) {
          <div class="empty-state">
            <mat-icon>bar_chart</mat-icon>
            <p>{{ 'calendar.planning.noData' | translate }}</p>
          </div>
        }
      }
    </div>
  `,
  styles: [`
    .planning-page {
      padding: 24px;
      max-width: 1400px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      margin-bottom: 24px;
    }

    .page-header h1 {
      display: flex;
      align-items: center;
      gap: 8px;
      margin: 0 0 4px;
    }

    .subtitle {
      color: #666;
      margin: 0;
    }

    .filter-card {
      margin-bottom: 24px;
    }

    .filter-form {
      display: flex;
      gap: 12px;
      flex-wrap: wrap;
      align-items: flex-start;
    }

    .filter-field {
      flex: 1 1 200px;
      min-width: 180px;
    }

    .filter-actions {
      padding-top: 4px;
    }

    .loading-container {
      display: flex;
      justify-content: center;
      padding: 80px;
    }

    /* Summary Cards */
    .summary-cards {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .summary-card mat-card-content {
      display: flex;
      align-items: center;
      gap: 16px;
      padding: 20px;
    }

    .summary-card mat-icon {
      font-size: 36px;
      width: 36px;
      height: 36px;
    }

    .summary-card.supply mat-icon { color: #4caf50; }
    .summary-card.demand mat-icon { color: #2196f3; }
    .summary-card.gap.surplus mat-icon { color: #4caf50; }
    .summary-card.gap.deficit mat-icon { color: #f44336; }

    .summary-info {
      display: flex;
      flex-direction: column;
    }

    .summary-value {
      font-size: 1.8rem;
      font-weight: 700;
    }

    .summary-label {
      font-size: 0.85rem;
      color: #666;
    }

    /* Race Timeline */
    .race-timeline-card {
      margin-bottom: 24px;
    }

    .race-icon {
      color: #2e7d32;
    }

    .chart-legend {
      display: flex;
      gap: 24px;
      margin-bottom: 16px;
    }

    .legend-item {
      display: flex;
      align-items: center;
      gap: 6px;
      font-size: 0.85rem;
    }

    .legend-dot {
      width: 12px;
      height: 12px;
      border-radius: 3px;
    }

    .supply-dot { background: #4caf50; }
    .demand-dot { background: #2196f3; }

    /* Timeline Chart */
    .timeline-chart {
      display: flex;
      gap: 8px;
      overflow-x: auto;
      padding-bottom: 8px;
    }

    .week-column {
      display: flex;
      flex-direction: column;
      align-items: center;
      min-width: 80px;
      flex: 1;
    }

    .bars-container {
      display: flex;
      gap: 4px;
      align-items: flex-end;
      height: 140px;
      width: 100%;
      justify-content: center;
    }

    .bar {
      width: 28px;
      min-height: 4px;
      border-radius: 4px 4px 0 0;
      display: flex;
      align-items: flex-start;
      justify-content: center;
      transition: height 0.3s ease;
    }

    .supply-bar { background: #4caf50; }
    .demand-bar { background: #2196f3; }

    .bar-value {
      font-size: 0.65rem;
      color: white;
      font-weight: 600;
      padding-top: 2px;
    }

    .week-label {
      font-size: 0.7rem;
      color: #888;
      margin-top: 4px;
      text-align: center;
      white-space: nowrap;
    }

    .gap-indicator {
      display: flex;
      align-items: center;
      gap: 2px;
      font-size: 0.7rem;
      font-weight: 600;
      margin-top: 4px;
      padding: 2px 6px;
      border-radius: 4px;
    }

    .gap-indicator mat-icon {
      font-size: 14px;
      width: 14px;
      height: 14px;
    }

    .gap-indicator.surplus {
      color: #2e7d32;
      background: #e8f5e9;
    }

    .gap-indicator.deficit {
      color: #c62828;
      background: #ffebee;
    }

    .gap-indicator.balanced {
      color: #666;
      background: #f5f5f5;
    }

    .empty-state {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 80px;
      color: #999;
    }

    .empty-state mat-icon {
      font-size: 64px;
      width: 64px;
      height: 64px;
      margin-bottom: 16px;
    }
  `],
})
export class PlanningComponent implements OnInit {
  private readonly calendarService = inject(CalendarService);
  private readonly fb = inject(FormBuilder);

  readonly races = CHICKEN_RACES;
  readonly planningData = signal<PlanningData[]>([]);
  readonly loading = signal(true);

  readonly filterForm: FormGroup = this.fb.group({
    race: [''],
    location: [''],
  });

  readonly totalSupply = computed(() =>
    this.planningData().reduce(
      (sum, rd) => sum + rd.weeks.reduce((ws, w) => ws + w.supply, 0),
      0,
    ),
  );

  readonly totalDemand = computed(() =>
    this.planningData().reduce(
      (sum, rd) => sum + rd.weeks.reduce((ws, w) => ws + w.demand, 0),
      0,
    ),
  );

  readonly totalGap = computed(() => this.totalSupply() - this.totalDemand());

  ngOnInit(): void {
    this.loadPlanningData();
  }

  applyFilters(): void {
    this.loadPlanningData();
  }

  getBarHeight(value: number, weeks: SupplyDemandWeek[]): number {
    const maxVal = Math.max(
      ...weeks.map(w => Math.max(w.supply, w.demand)),
      1,
    );
    return Math.max((value / maxVal) * 100, 3);
  }

  private loadPlanningData(): void {
    this.loading.set(true);
    const v = this.filterForm.value;
    const now = new Date();

    const filter: PlanningFilter = {
      dateFrom: now.toISOString(),
      dateTo: new Date(now.getFullYear(), now.getMonth() + 3, 0).toISOString(),
    };
    if (v.race) filter.race = v.race;
    if (v.location) filter.location = v.location;

    this.calendarService.getPlanningData(filter).subscribe({
      next: (data) => {
        this.planningData.set(data);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }
}
