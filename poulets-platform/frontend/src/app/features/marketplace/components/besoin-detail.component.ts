import { Component, OnInit, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatDividerModule } from '@angular/material/divider';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatTooltipModule } from '@angular/material/tooltip';
import { TranslateModule } from '@ngx-translate/core';

import { MarketplaceService } from '../services/marketplace.service';
import { Besoin, DAYS_OF_WEEK } from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-besoin-detail',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    MatDividerModule,
    MatProgressSpinnerModule,
    MatTooltipModule,
    TranslateModule,
  ],
  template: `
    <div class="besoin-detail-page">
      @if (loading()) {
        <div class="loading-container">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else if (besoin(); as b) {
        <!-- Breadcrumb -->
        <div class="breadcrumb">
          <a routerLink="/marketplace">{{ 'marketplace.title' | translate }}</a>
          <mat-icon>chevron_right</mat-icon>
          <a routerLink="/marketplace/besoins">{{ 'marketplace.besoins.title' | translate }}</a>
          <mat-icon>chevron_right</mat-icon>
          <span>{{ b.races.join(', ') }}</span>
        </div>

        <div class="detail-layout">
          <!-- Main Content -->
          <div class="main-content">
            <mat-card class="details-card">
              <mat-card-header>
                <mat-icon mat-card-avatar class="besoin-avatar">shopping_basket</mat-icon>
                <mat-card-title>
                  <h2>{{ b.races.join(', ') }} - {{ b.quantity }} {{ 'marketplace.besoin.units' | translate }}</h2>
                </mat-card-title>
              </mat-card-header>
              <mat-card-content>
                <!-- Badges -->
                <div class="status-badges">
                  <mat-chip-set>
                    <mat-chip [class]="'status-' + b.status.toLowerCase()">
                      {{ 'marketplace.besoin.status.' + b.status | translate }}
                    </mat-chip>
                    <mat-chip [class]="'freq-' + b.frequency.toLowerCase()">
                      {{ 'marketplace.frequency.' + b.frequency | translate }}
                    </mat-chip>
                    @if (b.halalRequired) {
                      <mat-chip class="badge-halal">
                        <mat-icon>check_circle</mat-icon>
                        {{ 'marketplace.besoin.halalRequired' | translate }}
                      </mat-chip>
                    }
                    @if (b.veterinaryCertifiedRequired) {
                      <mat-chip class="badge-vet">
                        <mat-icon>verified</mat-icon>
                        {{ 'marketplace.besoin.vetRequired' | translate }}
                      </mat-chip>
                    }
                  </mat-chip-set>
                </div>

                <mat-divider></mat-divider>

                <!-- Key Info -->
                <div class="info-grid">
                  <div class="info-item">
                    <mat-icon>inventory_2</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.besoin.quantity' | translate }}</span>
                      <span class="info-value">{{ b.quantity }}</span>
                    </div>
                  </div>
                  <div class="info-item">
                    <mat-icon>monitor_weight</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.besoin.minWeight' | translate }}</span>
                      <span class="info-value">{{ b.minimumWeight | number:'1.1-1' }} kg</span>
                    </div>
                  </div>
                  <div class="info-item">
                    <mat-icon>event</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.besoin.deliveryDate' | translate }}</span>
                      <span class="info-value">{{ b.deliveryDate | date:'mediumDate' }}</span>
                    </div>
                  </div>
                  <div class="info-item highlight">
                    <mat-icon>payments</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.besoin.maxBudget' | translate }}</span>
                      <span class="info-value price">{{ b.maxBudgetPerKg | number }} FCFA/kg</span>
                    </div>
                  </div>
                  <div class="info-item">
                    <mat-icon>location_on</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.besoin.location' | translate }}</span>
                      <span class="info-value">{{ b.location }}</span>
                    </div>
                  </div>
                  <div class="info-item">
                    <mat-icon>repeat</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.besoin.frequency' | translate }}</span>
                      <span class="info-value">{{ 'marketplace.frequency.' + b.frequency | translate }}</span>
                    </div>
                  </div>
                </div>

                <!-- Recurring Schedule -->
                @if (b.frequency !== 'PONCTUEL') {
                  <mat-divider></mat-divider>
                  <div class="recurring-section">
                    <h3>{{ 'marketplace.besoin.recurringSchedule' | translate }}</h3>
                    <div class="info-grid">
                      @if (b.recurringStartDate) {
                        <div class="info-item">
                          <mat-icon>play_arrow</mat-icon>
                          <div>
                            <span class="info-label">{{ 'marketplace.besoin.recurringStart' | translate }}</span>
                            <span class="info-value">{{ b.recurringStartDate | date:'mediumDate' }}</span>
                          </div>
                        </div>
                      }
                      @if (b.recurringEndDate) {
                        <div class="info-item">
                          <mat-icon>stop</mat-icon>
                          <div>
                            <span class="info-label">{{ 'marketplace.besoin.recurringEnd' | translate }}</span>
                            <span class="info-value">{{ b.recurringEndDate | date:'mediumDate' }}</span>
                          </div>
                        </div>
                      }
                      @if (b.dayOfWeekPreference != null) {
                        <div class="info-item">
                          <mat-icon>today</mat-icon>
                          <div>
                            <span class="info-label">{{ 'marketplace.besoin.dayPreference' | translate }}</span>
                            <span class="info-value">{{ getDayLabel(b.dayOfWeekPreference) }}</span>
                          </div>
                        </div>
                      }
                    </div>
                  </div>
                }

                <!-- Special Notes -->
                @if (b.specialNotes) {
                  <mat-divider></mat-divider>
                  <div class="notes-section">
                    <h3>{{ 'marketplace.besoin.specialNotes' | translate }}</h3>
                    <p>{{ b.specialNotes }}</p>
                  </div>
                }

                <!-- Actions -->
                <div class="action-buttons">
                  <button mat-raised-button color="primary" class="action-btn">
                    <mat-icon>handshake</mat-icon>
                    {{ 'marketplace.besoin.propose' | translate }}
                  </button>
                  <button mat-raised-button color="accent" class="action-btn">
                    <mat-icon>chat</mat-icon>
                    {{ 'marketplace.besoin.contact' | translate }}
                  </button>
                </div>
              </mat-card-content>
            </mat-card>
          </div>

          <!-- Sidebar: Client Info -->
          <div class="sidebar">
            <mat-card class="client-card">
              <mat-card-header>
                <mat-icon mat-card-avatar class="client-avatar">person</mat-icon>
                <mat-card-title>{{ b.client.nom }}</mat-card-title>
                <mat-card-subtitle>{{ b.client.localisation }}</mat-card-subtitle>
              </mat-card-header>
              <mat-card-content>
                <button mat-stroked-button class="full-width-btn">
                  <mat-icon>chat</mat-icon>
                  {{ 'marketplace.besoin.contactClient' | translate }}
                </button>
              </mat-card-content>
            </mat-card>

            <mat-card class="info-card">
              <mat-card-content>
                <div class="info-item-small">
                  <mat-icon>calendar_today</mat-icon>
                  <span>{{ 'marketplace.besoin.publishedOn' | translate }}: {{ b.createdAt | date:'mediumDate' }}</span>
                </div>
                @if (b.updatedAt) {
                  <div class="info-item-small">
                    <mat-icon>update</mat-icon>
                    <span>{{ 'marketplace.besoin.updatedOn' | translate }}: {{ b.updatedAt | date:'mediumDate' }}</span>
                  </div>
                }
              </mat-card-content>
            </mat-card>
          </div>
        </div>
      }
    </div>
  `,
  styles: [`
    .besoin-detail-page {
      padding: 24px;
      max-width: 1200px;
      margin: 0 auto;
    }

    .breadcrumb {
      display: flex;
      align-items: center;
      gap: 4px;
      margin-bottom: 24px;
      font-size: 0.9rem;
    }

    .breadcrumb a {
      color: #1976d2;
      text-decoration: none;
    }

    .breadcrumb a:hover {
      text-decoration: underline;
    }

    .breadcrumb mat-icon {
      font-size: 18px;
      width: 18px;
      height: 18px;
      color: #999;
    }

    .loading-container {
      display: flex;
      justify-content: center;
      padding: 80px;
    }

    .detail-layout {
      display: grid;
      grid-template-columns: 1fr 300px;
      gap: 24px;
    }

    @media (max-width: 960px) {
      .detail-layout {
        grid-template-columns: 1fr;
      }
    }

    .besoin-avatar {
      color: #1565c0;
      font-size: 28px;
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .status-badges {
      margin: 16px 0;
    }

    .status-active { --mdc-chip-elevated-container-color: #e8f5e9; }
    .status-satisfait { --mdc-chip-elevated-container-color: #e3f2fd; }
    .status-expire { --mdc-chip-elevated-container-color: #fafafa; }
    .status-annule { --mdc-chip-elevated-container-color: #ffebee; }
    .freq-ponctuel { --mdc-chip-elevated-container-color: #e8eaf6; }
    .freq-hebdomadaire { --mdc-chip-elevated-container-color: #e3f2fd; }
    .freq-bi_mensuel { --mdc-chip-elevated-container-color: #e0f2f1; }
    .freq-mensuel { --mdc-chip-elevated-container-color: #f3e5f5; }
    .badge-halal { --mdc-chip-elevated-container-color: #e3f2fd; }
    .badge-vet { --mdc-chip-elevated-container-color: #e8f5e9; }

    .info-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
      gap: 12px;
      margin: 16px 0;
    }

    .info-item {
      display: flex;
      align-items: flex-start;
      gap: 12px;
      padding: 10px;
      background: #fafafa;
      border-radius: 8px;
    }

    .info-item.highlight {
      background: #f1f8e9;
    }

    .info-item mat-icon {
      color: #666;
      margin-top: 2px;
    }

    .info-label {
      display: block;
      font-size: 0.75rem;
      color: #888;
    }

    .info-value {
      display: block;
      font-weight: 600;
    }

    .info-value.price {
      color: #2e7d32;
    }

    .recurring-section h3,
    .notes-section h3 {
      margin: 16px 0 8px;
    }

    .action-buttons {
      display: flex;
      gap: 12px;
      margin-top: 24px;
    }

    .action-btn {
      flex: 1;
      height: 48px;
    }

    /* Sidebar */
    .sidebar {
      display: flex;
      flex-direction: column;
      gap: 20px;
    }

    .client-avatar {
      color: #1976d2;
      background: #e3f2fd;
      font-size: 28px;
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: 50%;
    }

    .full-width-btn {
      width: 100%;
      margin-top: 12px;
    }

    .info-item-small {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 0.85rem;
      color: #666;
      margin: 8px 0;
    }

    .info-item-small mat-icon {
      font-size: 18px;
      width: 18px;
      height: 18px;
      color: #999;
    }
  `],
})
export class BesoinDetailComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);
  private readonly marketplace = inject(MarketplaceService);

  readonly loading = signal(true);
  readonly besoin = signal<Besoin | null>(null);
  private readonly daysOfWeek = DAYS_OF_WEEK;

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id')!;
    this.marketplace.getBesoinById(id).subscribe({
      next: (besoin) => {
        this.besoin.set(besoin);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }

  getDayLabel(dayValue: number): string {
    const day = this.daysOfWeek.find(d => d.value === dayValue);
    return day ? day.label : '';
  }
}
