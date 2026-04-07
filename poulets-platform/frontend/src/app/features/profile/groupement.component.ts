import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatTableModule } from '@angular/material/table';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';
import { RatingStarsComponent } from '@shared/components/rating-stars/rating-stars.component';
import { FcfaCurrencyPipe } from '@shared/pipes/currency.pipe';

interface GroupementMember {
  id: string;
  name: string;
  role: string;
  rating: number;
  totalVentes: number;
  joinedAt: string;
}

interface GroupementData {
  name: string;
  description: string;
  localisation: string;
  membersCount: number;
  totalAnnonces: number;
  totalRevenue: number;
  avgRating: number;
  createdAt: string;
  members: GroupementMember[];
}

@Component({
  selector: 'app-groupement',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatTableModule,
    MatDividerModule,
    TranslateModule,
    RatingStarsComponent,
    FcfaCurrencyPipe,
    DatePipe,
  ],
  template: `
    <div class="groupement-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'profile.groupement.title' | translate }}</h1>
      </div>

      @if (groupement(); as g) {
        <!-- Groupement Info -->
        <mat-card class="info-card">
          <mat-card-content>
            <div class="groupement-header">
              <div class="groupement-icon">
                <mat-icon>groups</mat-icon>
              </div>
              <div>
                <h2>{{ g.name }}</h2>
                <p class="groupement-location">
                  <mat-icon>location_on</mat-icon> {{ g.localisation }}
                </p>
              </div>
            </div>
            <p class="groupement-desc">{{ g.description }}</p>
          </mat-card-content>
        </mat-card>

        <!-- Stats -->
        <div class="stats-grid">
          <mat-card class="stat-card">
            <mat-card-content>
              <span class="stat-value">{{ g.membersCount }}</span>
              <span class="stat-label">{{ 'profile.groupement.members' | translate }}</span>
            </mat-card-content>
          </mat-card>
          <mat-card class="stat-card">
            <mat-card-content>
              <span class="stat-value">{{ g.totalAnnonces }}</span>
              <span class="stat-label">{{ 'profile.groupement.shared_annonces' | translate }}</span>
            </mat-card-content>
          </mat-card>
          <mat-card class="stat-card">
            <mat-card-content>
              <span class="stat-value">{{ g.totalRevenue | fcfa }}</span>
              <span class="stat-label">{{ 'profile.groupement.total_revenue' | translate }}</span>
            </mat-card-content>
          </mat-card>
          <mat-card class="stat-card">
            <mat-card-content>
              <app-rating-stars [value]="g.avgRating" [showValue]="true"></app-rating-stars>
              <span class="stat-label">{{ 'profile.groupement.avg_rating' | translate }}</span>
            </mat-card-content>
          </mat-card>
        </div>

        <!-- Members Table -->
        <mat-card>
          <mat-card-header>
            <mat-card-title>{{ 'profile.groupement.members_list' | translate }}</mat-card-title>
          </mat-card-header>
          <mat-card-content>
            <table mat-table [dataSource]="g.members" class="full-width-table">
              <ng-container matColumnDef="name">
                <th mat-header-cell *matHeaderCellDef>{{ 'profile.groupement.member_name' | translate }}</th>
                <td mat-cell *matCellDef="let m">{{ m.name }}</td>
              </ng-container>
              <ng-container matColumnDef="role">
                <th mat-header-cell *matHeaderCellDef>{{ 'profile.groupement.role' | translate }}</th>
                <td mat-cell *matCellDef="let m">{{ m.role }}</td>
              </ng-container>
              <ng-container matColumnDef="rating">
                <th mat-header-cell *matHeaderCellDef>{{ 'profile.groupement.rating' | translate }}</th>
                <td mat-cell *matCellDef="let m">
                  <app-rating-stars [value]="m.rating" [showValue]="true"></app-rating-stars>
                </td>
              </ng-container>
              <ng-container matColumnDef="totalVentes">
                <th mat-header-cell *matHeaderCellDef>{{ 'profile.groupement.sales' | translate }}</th>
                <td mat-cell *matCellDef="let m">{{ m.totalVentes }}</td>
              </ng-container>
              <ng-container matColumnDef="joinedAt">
                <th mat-header-cell *matHeaderCellDef>{{ 'profile.groupement.joined' | translate }}</th>
                <td mat-cell *matCellDef="let m">{{ m.joinedAt | date:'MM/yyyy' }}</td>
              </ng-container>
              <tr mat-header-row *matHeaderRowDef="memberColumns"></tr>
              <tr mat-row *matRowDef="let row; columns: memberColumns;"></tr>
            </table>
          </mat-card-content>
        </mat-card>
      }
    </div>
  `,
  styles: [`
    .groupement-container {
      padding: 24px;
      max-width: 900px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .info-card { margin-bottom: 24px; }

    .groupement-header {
      display: flex;
      align-items: center;
      gap: 16px;
      margin-bottom: 12px;

      h2 { margin: 0; }
    }

    .groupement-icon {
      width: 56px;
      height: 56px;
      border-radius: 50%;
      background: var(--faso-primary, #2e7d32);
      color: white;
      display: flex;
      align-items: center;
      justify-content: center;

      mat-icon { font-size: 28px; width: 28px; height: 28px; }
    }

    .groupement-location {
      display: flex;
      align-items: center;
      gap: 4px;
      color: #666;
      font-size: 0.9rem;
      margin: 4px 0 0;

      mat-icon { font-size: 18px; width: 18px; height: 18px; }
    }

    .groupement-desc {
      color: #555;
      line-height: 1.5;
    }

    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .stat-card mat-card-content {
      display: flex;
      flex-direction: column;
      align-items: center;
      text-align: center;

      .stat-value { font-size: 1.5rem; font-weight: 700; color: var(--faso-primary-dark, #1b5e20); }
      .stat-label { font-size: 0.8rem; color: #666; margin-top: 4px; }
    }

    .full-width-table { width: 100%; }
  `],
})
export class GroupementComponent implements OnInit {
  readonly groupement = signal<GroupementData | null>(null);
  readonly memberColumns = ['name', 'role', 'rating', 'totalVentes', 'joinedAt'];

  ngOnInit(): void {
    this.loadGroupement();
  }

  private loadGroupement(): void {
    this.groupement.set({
      name: 'Groupement des Eleveurs de Koudougou',
      description: 'Groupement cooperatif d\'eleveurs de volaille de la region de Koudougou. Nous mutualisons les achats d\'aliments, le suivi veterinaire et la commercialisation.',
      localisation: 'Koudougou, Burkina Faso',
      membersCount: 12,
      totalAnnonces: 34,
      totalRevenue: 4850000,
      avgRating: 4.3,
      createdAt: '2024-01-15',
      members: [
        { id: 'm1', name: 'Ouedraogo Moussa', role: 'President', rating: 4.4, totalVentes: 47, joinedAt: '2024-01-15' },
        { id: 'm2', name: 'Kabore Amidou', role: 'Tresorier', rating: 4.2, totalVentes: 32, joinedAt: '2024-02-01' },
        { id: 'm3', name: 'Traore Fatimata', role: 'Secretaire', rating: 4.6, totalVentes: 28, joinedAt: '2024-02-15' },
        { id: 'm4', name: 'Sawadogo Paul', role: 'Membre', rating: 4.0, totalVentes: 15, joinedAt: '2024-03-01' },
        { id: 'm5', name: 'Compaore Issa', role: 'Membre', rating: 4.1, totalVentes: 22, joinedAt: '2024-04-10' },
      ],
    });
  }
}
