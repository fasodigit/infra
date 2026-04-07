import { Component, OnInit, inject, signal, computed, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatTabsModule } from '@angular/material/tabs';
import { MatChipsModule } from '@angular/material/chips';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatPaginatorModule, PageEvent } from '@angular/material/paginator';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';

import { MarketplaceService } from '../services/marketplace.service';
import { AuthService } from '../../../services/auth.service';
import { MatchResult } from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-matching',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatTabsModule,
    MatChipsModule,
    MatProgressSpinnerModule,
    MatProgressBarModule,
    MatTooltipModule,
    MatPaginatorModule,
    MatDividerModule,
    TranslateModule,
  ],
  template: `
    <div class="matching-page">
      <div class="page-header">
        <h1>
          <mat-icon>compare_arrows</mat-icon>
          {{ 'marketplace.matching.title' | translate }}
        </h1>
        <p class="subtitle">{{ 'marketplace.matching.subtitle' | translate }}</p>
      </div>

      @if (loading()) {
        <div class="loading-container">
          <mat-spinner diameter="48"></mat-spinner>
          <p>{{ 'marketplace.matching.loading' | translate }}</p>
        </div>
      } @else if (matches().length === 0) {
        <div class="empty-state">
          <mat-icon>search_off</mat-icon>
          <h3>{{ 'marketplace.matching.noMatches' | translate }}</h3>
          <p>{{ 'marketplace.matching.noMatchesHint' | translate }}</p>
          @if (isEleveur()) {
            <a mat-raised-button color="primary" routerLink="/marketplace/annonces/new">
              <mat-icon>add</mat-icon>
              {{ 'marketplace.matching.publishAnnonce' | translate }}
            </a>
          } @else {
            <a mat-raised-button color="primary" routerLink="/marketplace/besoins/new">
              <mat-icon>add</mat-icon>
              {{ 'marketplace.matching.publishBesoin' | translate }}
            </a>
          }
        </div>
      } @else {
        <div class="matches-list">
          @for (match of matches(); track match.id) {
            <mat-card class="match-card">
              <!-- Score Header -->
              <div class="match-score-header"
                [class.excellent]="match.matchScore >= 80"
                [class.good]="match.matchScore >= 60 && match.matchScore < 80"
                [class.moderate]="match.matchScore < 60">
                <div class="score-circle">
                  <span class="score-value">{{ match.matchScore }}%</span>
                </div>
                <div class="score-label">{{ 'marketplace.matching.matchScore' | translate }}</div>
              </div>

              <mat-card-content>
                <!-- Match target info -->
                @if (isEleveur() && match.besoin) {
                  <div class="match-target">
                    <h3>
                      <mat-icon>shopping_basket</mat-icon>
                      {{ match.besoin.races.join(', ') }}
                    </h3>
                    <p class="target-subtitle">
                      {{ match.besoin.client.nom }} - {{ match.besoin.location }}
                    </p>
                    <div class="target-details">
                      <div class="detail-chip">
                        <mat-icon>inventory_2</mat-icon>
                        {{ match.besoin.quantity }} {{ 'marketplace.matching.units' | translate }}
                      </div>
                      <div class="detail-chip">
                        <mat-icon>monitor_weight</mat-icon>
                        {{ match.besoin.minimumWeight | number:'1.1-1' }} kg min
                      </div>
                      <div class="detail-chip">
                        <mat-icon>event</mat-icon>
                        {{ match.besoin.deliveryDate | date:'shortDate' }}
                      </div>
                      <div class="detail-chip price">
                        <mat-icon>payments</mat-icon>
                        {{ match.besoin.maxBudgetPerKg | number }} FCFA/kg
                      </div>
                      <div class="detail-chip freq">
                        <mat-icon>repeat</mat-icon>
                        {{ 'marketplace.frequency.' + match.besoin.frequency | translate }}
                      </div>
                    </div>
                  </div>
                } @else if (!isEleveur() && match.annonce) {
                  <div class="match-target">
                    <h3>
                      <mat-icon>egg_alt</mat-icon>
                      {{ match.annonce.race }}
                    </h3>
                    <p class="target-subtitle">
                      {{ match.annonce.eleveur.nom }} - {{ match.annonce.location }}
                      <span class="rating">
                        <mat-icon>star</mat-icon>
                        {{ match.annonce.eleveur.note | number:'1.1-1' }}
                      </span>
                    </p>
                    <div class="target-details">
                      <div class="detail-chip">
                        <mat-icon>inventory_2</mat-icon>
                        {{ match.annonce.quantity }} {{ 'marketplace.matching.units' | translate }}
                      </div>
                      <div class="detail-chip">
                        <mat-icon>monitor_weight</mat-icon>
                        {{ match.annonce.currentWeight | number:'1.1-1' }} - {{ match.annonce.estimatedWeight | number:'1.1-1' }} kg
                      </div>
                      <div class="detail-chip price">
                        <mat-icon>payments</mat-icon>
                        {{ match.annonce.pricePerKg | number }} FCFA/kg
                      </div>
                      @if (match.annonce.halalCertified) {
                        <div class="detail-chip halal">
                          <mat-icon>check_circle</mat-icon>
                          {{ 'marketplace.matching.halal' | translate }}
                        </div>
                      }
                      @if (match.annonce.veterinaryStatus === 'VERIFIED') {
                        <div class="detail-chip vet">
                          <mat-icon>verified</mat-icon>
                          {{ 'marketplace.matching.vetVerified' | translate }}
                        </div>
                      }
                    </div>
                  </div>
                }

                <mat-divider></mat-divider>

                <!-- Score Breakdown -->
                <div class="score-breakdown">
                  <h4>{{ 'marketplace.matching.breakdown' | translate }}</h4>
                  <div class="breakdown-items">
                    <div class="breakdown-item">
                      <div class="breakdown-header">
                        <span>{{ 'marketplace.matching.raceCompat' | translate }}</span>
                        <span class="breakdown-value">{{ match.raceCompatibility }}%</span>
                      </div>
                      <mat-progress-bar mode="determinate" [value]="match.raceCompatibility"
                        [color]="match.raceCompatibility >= 70 ? 'primary' : 'warn'">
                      </mat-progress-bar>
                    </div>
                    <div class="breakdown-item">
                      <div class="breakdown-header">
                        <span>{{ 'marketplace.matching.weightFeasibility' | translate }}</span>
                        <span class="breakdown-value">{{ match.weightFeasibility }}%</span>
                      </div>
                      <mat-progress-bar mode="determinate" [value]="match.weightFeasibility"
                        [color]="match.weightFeasibility >= 70 ? 'primary' : 'warn'">
                      </mat-progress-bar>
                    </div>
                    <div class="breakdown-item">
                      <div class="breakdown-header">
                        <span>{{ 'marketplace.matching.dateCompat' | translate }}</span>
                        <span class="breakdown-value">{{ match.dateCompatibility }}%</span>
                      </div>
                      <mat-progress-bar mode="determinate" [value]="match.dateCompatibility"
                        [color]="match.dateCompatibility >= 70 ? 'primary' : 'warn'">
                      </mat-progress-bar>
                    </div>
                    <div class="breakdown-item">
                      <div class="breakdown-header">
                        <span>{{ 'marketplace.matching.proximity' | translate }}</span>
                        <span class="breakdown-value">{{ match.proximity }}%</span>
                      </div>
                      <mat-progress-bar mode="determinate" [value]="match.proximity"
                        [color]="match.proximity >= 70 ? 'primary' : 'warn'">
                      </mat-progress-bar>
                    </div>
                    <div class="breakdown-item">
                      <div class="breakdown-header">
                        <span>{{ 'marketplace.matching.reputation' | translate }}</span>
                        <span class="breakdown-value">{{ match.reputation }}%</span>
                      </div>
                      <mat-progress-bar mode="determinate" [value]="match.reputation"
                        [color]="match.reputation >= 70 ? 'primary' : 'warn'">
                      </mat-progress-bar>
                    </div>
                  </div>
                </div>

                <!-- Actions -->
                <div class="match-actions">
                  @if (isEleveur() && match.besoin) {
                    <a mat-raised-button color="primary"
                      [routerLink]="['/marketplace/besoins', match.besoin.id]">
                      <mat-icon>visibility</mat-icon>
                      {{ 'marketplace.matching.viewBesoin' | translate }}
                    </a>
                  } @else if (!isEleveur() && match.annonce) {
                    <a mat-raised-button color="primary"
                      [routerLink]="['/marketplace/annonces', match.annonce.id]">
                      <mat-icon>visibility</mat-icon>
                      {{ 'marketplace.matching.viewAnnonce' | translate }}
                    </a>
                  }
                  <button mat-raised-button color="accent">
                    <mat-icon>chat</mat-icon>
                    {{ 'marketplace.matching.contact' | translate }}
                  </button>
                </div>
              </mat-card-content>
            </mat-card>
          }
        </div>

        <mat-paginator
          [length]="totalElements()"
          [pageSize]="pageSize"
          [pageIndex]="currentPage()"
          [pageSizeOptions]="[10, 20, 50]"
          (page)="onPageChange($event)"
          showFirstLastButtons>
        </mat-paginator>
      }
    </div>
  `,
  styles: [`
    .matching-page {
      padding: 24px;
      max-width: 1000px;
      margin: 0 auto;
    }

    .page-header h1 {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 4px;
    }

    .subtitle {
      color: #666;
      margin-top: 0;
    }

    .loading-container {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 80px;
      color: #666;
    }

    .loading-container p {
      margin-top: 16px;
    }

    .empty-state {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 80px;
      color: #999;
      text-align: center;
    }

    .empty-state mat-icon {
      font-size: 72px;
      width: 72px;
      height: 72px;
      margin-bottom: 16px;
    }

    .empty-state h3 {
      margin-bottom: 8px;
    }

    .empty-state p {
      margin-bottom: 20px;
    }

    .matches-list {
      display: flex;
      flex-direction: column;
      gap: 20px;
      margin-bottom: 24px;
    }

    .match-card {
      overflow: hidden;
    }

    .match-score-header {
      display: flex;
      align-items: center;
      gap: 16px;
      padding: 16px 24px;
      color: white;
    }

    .match-score-header.excellent { background: linear-gradient(135deg, #2e7d32, #4caf50); }
    .match-score-header.good { background: linear-gradient(135deg, #1565c0, #42a5f5); }
    .match-score-header.moderate { background: linear-gradient(135deg, #e65100, #ff9800); }

    .score-circle {
      width: 56px;
      height: 56px;
      border-radius: 50%;
      background: rgba(255, 255, 255, 0.2);
      display: flex;
      align-items: center;
      justify-content: center;
      border: 3px solid rgba(255, 255, 255, 0.6);
    }

    .score-value {
      font-size: 1.2rem;
      font-weight: 700;
    }

    .score-label {
      font-size: 1.1rem;
      font-weight: 500;
    }

    .match-target {
      padding: 16px 0;
    }

    .match-target h3 {
      display: flex;
      align-items: center;
      gap: 8px;
      margin: 0 0 4px;
    }

    .target-subtitle {
      color: #666;
      margin: 0 0 12px;
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .rating {
      display: inline-flex;
      align-items: center;
      gap: 2px;
      color: #ff9800;
      font-weight: 500;
    }

    .rating mat-icon {
      font-size: 16px;
      width: 16px;
      height: 16px;
    }

    .target-details {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }

    .detail-chip {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      padding: 4px 12px;
      background: #f5f5f5;
      border-radius: 16px;
      font-size: 0.85rem;
    }

    .detail-chip mat-icon {
      font-size: 16px;
      width: 16px;
      height: 16px;
      color: #666;
    }

    .detail-chip.price { background: #e8f5e9; color: #2e7d32; }
    .detail-chip.price mat-icon { color: #2e7d32; }
    .detail-chip.freq { background: #e3f2fd; color: #1565c0; }
    .detail-chip.freq mat-icon { color: #1565c0; }
    .detail-chip.halal { background: #e3f2fd; color: #1565c0; }
    .detail-chip.halal mat-icon { color: #1565c0; }
    .detail-chip.vet { background: #e8f5e9; color: #2e7d32; }
    .detail-chip.vet mat-icon { color: #2e7d32; }

    mat-divider {
      margin: 16px 0;
    }

    .score-breakdown h4 {
      margin: 0 0 12px;
      color: #333;
    }

    .breakdown-items {
      display: flex;
      flex-direction: column;
      gap: 10px;
    }

    .breakdown-item {
      width: 100%;
    }

    .breakdown-header {
      display: flex;
      justify-content: space-between;
      margin-bottom: 4px;
      font-size: 0.85rem;
    }

    .breakdown-value {
      font-weight: 600;
    }

    .match-actions {
      display: flex;
      gap: 12px;
      margin-top: 20px;
    }

    @media (max-width: 600px) {
      .match-actions {
        flex-direction: column;
      }
    }
  `],
})
export class MatchingComponent implements OnInit {
  private readonly marketplace = inject(MarketplaceService);
  private readonly auth = inject(AuthService);

  readonly matches = signal<MatchResult[]>([]);
  readonly loading = signal(true);
  readonly totalElements = signal(0);
  readonly currentPage = signal(0);
  readonly pageSize = 20;

  isEleveur(): boolean {
    return this.auth.isEleveur();
  }

  ngOnInit(): void {
    this.loadMatches();
  }

  onPageChange(event: PageEvent): void {
    this.currentPage.set(event.pageIndex);
    this.loadMatches();
  }

  private loadMatches(): void {
    this.loading.set(true);
    const obs = this.isEleveur()
      ? this.marketplace.getMatchesForEleveur(this.currentPage(), this.pageSize)
      : this.marketplace.getMatchesForClient(this.currentPage(), this.pageSize);

    obs.subscribe({
      next: (page) => {
        this.matches.set(page.content);
        this.totalElements.set(page.totalElements);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }
}
