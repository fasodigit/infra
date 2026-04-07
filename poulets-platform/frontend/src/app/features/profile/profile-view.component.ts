import { Component, OnInit, signal, inject } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatDividerModule } from '@angular/material/divider';
import { MatChipsModule } from '@angular/material/chips';
import { TranslateModule } from '@ngx-translate/core';
import { RatingStarsComponent } from '@shared/components/rating-stars/rating-stars.component';
import { AuthService } from '@services/auth.service';

interface ProfileData {
  name: string;
  email: string;
  phone: string;
  role: string;
  avatar?: string;
  localisation: string;
  groupement?: string;
  memberSince: string;
  verified: boolean;
  totalTransactions: number;
  avgRating: number;
  totalReviews: number;
  description?: string;
}

@Component({
  selector: 'app-profile-view',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatDividerModule,
    MatChipsModule,
    TranslateModule,
    RatingStarsComponent,
    DatePipe,
  ],
  template: `
    <div class="profile-container">
      <div class="page-header">
        <h1>{{ 'profile.view.title' | translate }}</h1>
        <a mat-raised-button color="primary" routerLink="edit">
          <mat-icon>edit</mat-icon>
          {{ 'profile.view.edit' | translate }}
        </a>
      </div>

      @if (profile(); as p) {
        <!-- Profile Card -->
        <mat-card class="profile-card">
          <mat-card-content>
            <div class="profile-header">
              <div class="avatar-large">
                @if (p.avatar) {
                  <img [src]="p.avatar" [alt]="p.name">
                } @else {
                  {{ getInitials(p.name) }}
                }
              </div>
              <div class="profile-info">
                <h2>
                  {{ p.name }}
                  @if (p.verified) {
                    <mat-icon class="verified-icon">verified</mat-icon>
                  }
                </h2>
                <mat-chip class="role-chip">{{ p.role }}</mat-chip>
              </div>
            </div>

            <mat-divider></mat-divider>

            <div class="details-grid">
              <div class="detail-item">
                <mat-icon>email</mat-icon>
                <div>
                  <span class="detail-label">{{ 'profile.view.email' | translate }}</span>
                  <span class="detail-value">{{ p.email }}</span>
                </div>
              </div>
              <div class="detail-item">
                <mat-icon>phone</mat-icon>
                <div>
                  <span class="detail-label">{{ 'profile.view.phone' | translate }}</span>
                  <span class="detail-value">{{ p.phone }}</span>
                </div>
              </div>
              <div class="detail-item">
                <mat-icon>location_on</mat-icon>
                <div>
                  <span class="detail-label">{{ 'profile.view.location' | translate }}</span>
                  <span class="detail-value">{{ p.localisation }}</span>
                </div>
              </div>
              @if (p.groupement) {
                <div class="detail-item">
                  <mat-icon>groups</mat-icon>
                  <div>
                    <span class="detail-label">{{ 'profile.view.groupement' | translate }}</span>
                    <a class="detail-value groupement-link" routerLink="groupement">{{ p.groupement }}</a>
                  </div>
                </div>
              }
              <div class="detail-item">
                <mat-icon>calendar_today</mat-icon>
                <div>
                  <span class="detail-label">{{ 'profile.view.member_since' | translate }}</span>
                  <span class="detail-value">{{ p.memberSince | date:'MMMM yyyy' }}</span>
                </div>
              </div>
            </div>

            @if (p.description) {
              <mat-divider></mat-divider>
              <div class="description-section">
                <h3>{{ 'profile.view.description' | translate }}</h3>
                <p>{{ p.description }}</p>
              </div>
            }
          </mat-card-content>
        </mat-card>

        <!-- Stats -->
        <div class="stats-grid">
          <mat-card class="stat-card">
            <mat-card-content>
              <mat-icon>receipt_long</mat-icon>
              <span class="stat-value">{{ p.totalTransactions }}</span>
              <span class="stat-label">{{ 'profile.view.total_transactions' | translate }}</span>
            </mat-card-content>
          </mat-card>
          <mat-card class="stat-card">
            <mat-card-content>
              <div class="stat-rating">
                <app-rating-stars [value]="p.avgRating" [showValue]="true"
                                  [showCount]="true" [count]="p.totalReviews">
                </app-rating-stars>
              </div>
              <span class="stat-label">{{ 'profile.view.avg_rating' | translate }}</span>
            </mat-card-content>
          </mat-card>
        </div>
      }
    </div>
  `,
  styles: [`
    .profile-container {
      padding: 24px;
      max-width: 800px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .profile-card { margin-bottom: 24px; }

    .profile-header {
      display: flex;
      align-items: center;
      gap: 24px;
      padding: 16px 0;
    }

    .avatar-large {
      width: 80px;
      height: 80px;
      border-radius: 50%;
      background: var(--faso-primary, #2e7d32);
      color: white;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 1.8rem;
      font-weight: 700;
      overflow: hidden;

      img { width: 100%; height: 100%; object-fit: cover; }
    }

    .profile-info {
      h2 {
        margin: 0;
        display: flex;
        align-items: center;
        gap: 8px;

        .verified-icon { color: #2196f3; font-size: 22px; }
      }
    }

    .role-chip {
      margin-top: 4px;
      text-transform: capitalize;
    }

    .details-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
      padding: 16px 0;
    }

    .detail-item {
      display: flex;
      align-items: flex-start;
      gap: 12px;

      mat-icon { color: var(--faso-primary, #2e7d32); margin-top: 2px; }
      div { display: flex; flex-direction: column; }
      .detail-label { font-size: 0.75rem; color: #999; }
      .detail-value { font-weight: 500; }

      .groupement-link {
        color: var(--faso-primary, #2e7d32);
        text-decoration: none;
        &:hover { text-decoration: underline; }
      }
    }

    .description-section {
      padding: 16px 0;

      h3 { margin: 0 0 8px; font-size: 0.95rem; }
      p { margin: 0; font-size: 0.9rem; color: #555; line-height: 1.5; }
    }

    .stats-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
    }

    .stat-card mat-card-content {
      display: flex;
      flex-direction: column;
      align-items: center;
      text-align: center;
      gap: 8px;

      mat-icon {
        font-size: 32px;
        width: 32px;
        height: 32px;
        color: var(--faso-primary, #2e7d32);
      }

      .stat-value { font-size: 2rem; font-weight: 700; }
      .stat-label { font-size: 0.85rem; color: #666; }
    }

    @media (max-width: 600px) {
      .details-grid { grid-template-columns: 1fr; }
      .stats-grid { grid-template-columns: 1fr; }
    }
  `],
})
export class ProfileViewComponent implements OnInit {
  private readonly auth = inject(AuthService);
  readonly profile = signal<ProfileData | null>(null);

  ngOnInit(): void {
    this.loadProfile();
  }

  getInitials(name: string): string {
    return name.split(' ').map(n => n[0]).join('').substring(0, 2).toUpperCase();
  }

  private loadProfile(): void {
    const user = this.auth.currentUser();
    this.profile.set({
      name: user?.name || 'Ouedraogo Moussa',
      email: user?.email || 'moussa.ouedraogo@example.com',
      phone: '+226 70 12 34 56',
      role: user?.role || 'eleveur',
      localisation: 'Koudougou, Burkina Faso',
      groupement: 'Groupement des Eleveurs de Koudougou',
      memberSince: '2025-06-01',
      verified: true,
      totalTransactions: 47,
      avgRating: 4.4,
      totalReviews: 23,
      description: 'Eleveur professionnel specialise dans le poulet bicyclette et la pintade. Plus de 5 ans d\'experience dans l\'aviculture traditionnelle amelioree.',
    });
  }
}
