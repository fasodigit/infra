import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';
import { RatingStarsComponent } from '@shared/components/rating-stars/rating-stars.component';

interface Badge {
  icon: string;
  label: string;
  color: string;
  earned: boolean;
}

interface Review {
  id: string;
  reviewerName: string;
  reviewerRole: string;
  rating: number;
  comment: string;
  date: string;
  aspects: { quality: number; punctuality: number; communication: number; weightAccuracy: number };
}

@Component({
  selector: 'app-reputation-view',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    MatDividerModule,
    TranslateModule,
    RatingStarsComponent,
    DatePipe,
  ],
  template: `
    <div class="reputation-container">
      <div class="page-header">
        <h1>{{ 'reputation.view.title' | translate }}</h1>
      </div>

      <!-- Overview Card -->
      <mat-card class="overview-card">
        <mat-card-content>
          <div class="overview-grid">
            <div class="rating-overview">
              <span class="big-rating">{{ avgRating().toFixed(1) }}</span>
              <app-rating-stars [value]="avgRating()" [showCount]="true" [count]="totalReviews()"></app-rating-stars>
              <span class="total-reviews">{{ totalReviews() }} {{ 'reputation.view.reviews' | translate }}</span>
            </div>

            <div class="rating-breakdown">
              @for (bar of ratingBars(); track bar.stars) {
                <div class="rating-bar">
                  <span class="bar-label">{{ bar.stars }}</span>
                  <mat-icon class="bar-star">star</mat-icon>
                  <div class="bar-track">
                    <div class="bar-fill" [style.width.%]="bar.percentage"></div>
                  </div>
                  <span class="bar-count">{{ bar.count }}</span>
                </div>
              }
            </div>
          </div>
        </mat-card-content>
      </mat-card>

      <!-- Badges -->
      <mat-card class="badges-card">
        <mat-card-header>
          <mat-card-title>{{ 'reputation.view.badges' | translate }}</mat-card-title>
        </mat-card-header>
        <mat-card-content>
          <div class="badges-grid">
            @for (badge of badges(); track badge.label) {
              <div class="badge-item" [class.earned]="badge.earned">
                <mat-icon [style.color]="badge.earned ? badge.color : '#ccc'">
                  {{ badge.icon }}
                </mat-icon>
                <span class="badge-label">{{ badge.label | translate }}</span>
              </div>
            }
          </div>
        </mat-card-content>
      </mat-card>

      <!-- Reviews List -->
      <mat-card>
        <mat-card-header>
          <mat-card-title>{{ 'reputation.view.all_reviews' | translate }}</mat-card-title>
        </mat-card-header>
        <mat-card-content>
          @for (review of reviews(); track review.id) {
            <div class="review-item">
              <div class="review-header">
                <div class="reviewer-info">
                  <div class="reviewer-avatar">{{ getInitials(review.reviewerName) }}</div>
                  <div>
                    <span class="reviewer-name">{{ review.reviewerName }}</span>
                    <span class="reviewer-role">{{ review.reviewerRole }}</span>
                  </div>
                </div>
                <div class="review-meta">
                  <app-rating-stars [value]="review.rating"></app-rating-stars>
                  <span class="review-date">{{ review.date | date:'dd/MM/yyyy' }}</span>
                </div>
              </div>
              <p class="review-comment">{{ review.comment }}</p>
              <div class="review-aspects">
                <span class="aspect">
                  {{ 'reputation.aspect.quality' | translate }}: {{ review.aspects.quality }}/5
                </span>
                <span class="aspect">
                  {{ 'reputation.aspect.punctuality' | translate }}: {{ review.aspects.punctuality }}/5
                </span>
                <span class="aspect">
                  {{ 'reputation.aspect.communication' | translate }}: {{ review.aspects.communication }}/5
                </span>
                <span class="aspect">
                  {{ 'reputation.aspect.weight_accuracy' | translate }}: {{ review.aspects.weightAccuracy }}/5
                </span>
              </div>
              <mat-divider></mat-divider>
            </div>
          }
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .reputation-container {
      padding: 24px;
      max-width: 900px;
      margin: 0 auto;
    }

    .page-header {
      margin-bottom: 24px;
      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .overview-card { margin-bottom: 24px; }

    .overview-grid {
      display: grid;
      grid-template-columns: 1fr 2fr;
      gap: 32px;
      align-items: center;
    }

    .rating-overview {
      display: flex;
      flex-direction: column;
      align-items: center;
      text-align: center;

      .big-rating {
        font-size: 3rem;
        font-weight: 700;
        color: var(--faso-primary-dark, #1b5e20);
        line-height: 1;
      }

      .total-reviews {
        font-size: 0.85rem;
        color: #666;
        margin-top: 4px;
      }
    }

    .rating-breakdown {
      display: flex;
      flex-direction: column;
      gap: 6px;
    }

    .rating-bar {
      display: flex;
      align-items: center;
      gap: 6px;

      .bar-label { font-size: 0.85rem; width: 12px; text-align: right; }
      .bar-star { font-size: 16px; width: 16px; height: 16px; color: #ff9800; }

      .bar-track {
        flex: 1;
        height: 8px;
        background: #f0f0f0;
        border-radius: 4px;
        overflow: hidden;
      }

      .bar-fill {
        height: 100%;
        background: #ff9800;
        border-radius: 4px;
        transition: width 0.6s ease;
      }

      .bar-count { font-size: 0.8rem; color: #999; width: 20px; }
    }

    .badges-card { margin-bottom: 24px; }

    .badges-grid {
      display: flex;
      gap: 24px;
      flex-wrap: wrap;
      padding: 8px 0;
    }

    .badge-item {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 6px;
      opacity: 0.4;
      transition: opacity 0.3s;

      &.earned { opacity: 1; }

      mat-icon { font-size: 36px; width: 36px; height: 36px; }
      .badge-label { font-size: 0.8rem; font-weight: 500; text-align: center; }
    }

    .review-item {
      padding: 16px 0;
    }

    .review-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 8px;
    }

    .reviewer-info {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .reviewer-avatar {
      width: 40px;
      height: 40px;
      border-radius: 50%;
      background: #e0e0e0;
      display: flex;
      align-items: center;
      justify-content: center;
      font-weight: 600;
      font-size: 0.85rem;
    }

    .reviewer-name { font-weight: 600; display: block; }
    .reviewer-role { font-size: 0.8rem; color: #666; display: block; }

    .review-meta {
      display: flex;
      flex-direction: column;
      align-items: flex-end;
      gap: 4px;

      .review-date { font-size: 0.75rem; color: #999; }
    }

    .review-comment {
      margin: 8px 0;
      font-size: 0.9rem;
      line-height: 1.5;
      color: #444;
    }

    .review-aspects {
      display: flex;
      gap: 16px;
      flex-wrap: wrap;
      margin-bottom: 12px;

      .aspect {
        font-size: 0.75rem;
        color: #888;
        background: #f5f5f5;
        padding: 2px 8px;
        border-radius: 4px;
      }
    }

    @media (max-width: 600px) {
      .overview-grid { grid-template-columns: 1fr; }
    }
  `],
})
export class ReputationViewComponent implements OnInit {
  readonly avgRating = signal(4.4);
  readonly totalReviews = signal(23);
  readonly ratingBars = signal<{ stars: number; count: number; percentage: number }[]>([]);
  readonly badges = signal<Badge[]>([]);
  readonly reviews = signal<Review[]>([]);

  ngOnInit(): void {
    this.loadData();
  }

  getInitials(name: string): string {
    return name.split(' ').map(n => n[0]).join('').substring(0, 2).toUpperCase();
  }

  private loadData(): void {
    const totalR = 23;
    const counts = [12, 7, 2, 1, 1]; // 5 to 1 star
    this.ratingBars.set(
      counts.map((count, i) => ({
        stars: 5 - i,
        count,
        percentage: (count / totalR) * 100,
      }))
    );

    this.badges.set([
      { icon: 'verified_user', label: 'reputation.badge.verified', color: '#2196f3', earned: true },
      { icon: 'schedule', label: 'reputation.badge.punctual', color: '#4caf50', earned: true },
      { icon: 'verified', label: 'reputation.badge.halal', color: '#9c27b0', earned: true },
      { icon: 'star', label: 'reputation.badge.five_stars', color: '#ff9800', earned: false },
    ]);

    this.reviews.set([
      {
        id: 'r1', reviewerName: 'Restaurant Le Sahel', reviewerRole: 'Client',
        rating: 5, comment: 'Excellent eleveur. Poulets de tres bonne qualite, livraison toujours a l\'heure. Je recommande vivement.',
        date: '2026-04-02', aspects: { quality: 5, punctuality: 5, communication: 5, weightAccuracy: 4 },
      },
      {
        id: 'r2', reviewerName: 'Hotel Splendide', reviewerRole: 'Client',
        rating: 4, comment: 'Bonne qualite globale. Le poids etait legerement inferieur a la commande mais service agreable.',
        date: '2026-03-20', aspects: { quality: 4, punctuality: 4, communication: 5, weightAccuracy: 3 },
      },
      {
        id: 'r3', reviewerName: 'Mme Traore', reviewerRole: 'Client',
        rating: 5, comment: 'Les pintades etaient superbes. Parfait pour notre ceremonie. Merci !',
        date: '2026-03-10', aspects: { quality: 5, punctuality: 5, communication: 4, weightAccuracy: 5 },
      },
      {
        id: 'r4', reviewerName: 'M. Kabore', reviewerRole: 'Client',
        rating: 4, comment: 'Bon rapport qualite-prix. Communication rapide et professionnelle.',
        date: '2026-02-28', aspects: { quality: 4, punctuality: 4, communication: 5, weightAccuracy: 4 },
      },
    ]);
  }
}
