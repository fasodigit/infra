import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatDividerModule } from '@angular/material/divider';

import { PouletService } from '@services/poulet.service';
import { AuthService } from '@services/auth.service';
import { EleveurStats } from '@services/graphql.service';

@Component({
  selector: 'app-eleveur-dashboard',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatProgressSpinnerModule,
    MatDividerModule,
  ],
  template: `
    <div class="container">
      <div class="page-header">
        <h1>Tableau de Bord Eleveur</h1>
        <p>Bienvenue, {{ auth.currentUser()?.name }}</p>
      </div>

      @if (loading()) {
        <div class="loading-overlay">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else if (stats()) {
        <!-- Stats Cards -->
        <div class="stats-grid">
          <mat-card class="stat-card">
            <mat-card-content>
              <div class="stat-icon poulets">
                <mat-icon>egg_alt</mat-icon>
              </div>
              <div class="stat-info">
                <span class="stat-value">{{ stats()!.totalPoulets }}</span>
                <span class="stat-label">Total Poulets</span>
              </div>
            </mat-card-content>
          </mat-card>

          <mat-card class="stat-card">
            <mat-card-content>
              <div class="stat-icon available">
                <mat-icon>check_circle</mat-icon>
              </div>
              <div class="stat-info">
                <span class="stat-value">{{ stats()!.pouletsDisponibles }}</span>
                <span class="stat-label">Disponibles</span>
              </div>
            </mat-card-content>
          </mat-card>

          <mat-card class="stat-card">
            <mat-card-content>
              <div class="stat-icon sold">
                <mat-icon>sell</mat-icon>
              </div>
              <div class="stat-info">
                <span class="stat-value">{{ stats()!.pouletsVendus }}</span>
                <span class="stat-label">Vendus</span>
              </div>
            </mat-card-content>
          </mat-card>

          <mat-card class="stat-card">
            <mat-card-content>
              <div class="stat-icon revenue">
                <mat-icon>payments</mat-icon>
              </div>
              <div class="stat-info">
                <span class="stat-value">{{ stats()!.chiffreAffaires | number:'1.0-0' }}</span>
                <span class="stat-label">Chiffre d'affaires (FCFA)</span>
              </div>
            </mat-card-content>
          </mat-card>

          <mat-card class="stat-card">
            <mat-card-content>
              <div class="stat-icon orders">
                <mat-icon>local_shipping</mat-icon>
              </div>
              <div class="stat-info">
                <span class="stat-value">{{ stats()!.commandesEnCours }}</span>
                <span class="stat-label">Commandes en cours</span>
              </div>
            </mat-card-content>
          </mat-card>

          <mat-card class="stat-card">
            <mat-card-content>
              <div class="stat-icon rating">
                <mat-icon>star</mat-icon>
              </div>
              <div class="stat-info">
                <span class="stat-value">{{ stats()!.noteMoyenne | number:'1.1-1' }}</span>
                <span class="stat-label">Note moyenne</span>
              </div>
            </mat-card-content>
          </mat-card>
        </div>

        <!-- Quick Actions -->
        <mat-divider class="section-divider"></mat-divider>
        <h2 class="section-title">Actions rapides</h2>
        <div class="actions-grid">
          <mat-card class="action-card" routerLink="/eleveur/poulets">
            <mat-card-content>
              <mat-icon>add_circle</mat-icon>
              <h3>Gerer mes poulets</h3>
              <p>Ajouter, modifier ou retirer des poulets du catalogue</p>
            </mat-card-content>
          </mat-card>

          <mat-card class="action-card">
            <mat-card-content>
              <mat-icon>inventory</mat-icon>
              <h3>Commandes recues</h3>
              <p>Voir et traiter les commandes de vos clients</p>
            </mat-card-content>
          </mat-card>

          <mat-card class="action-card">
            <mat-card-content>
              <mat-icon>bar_chart</mat-icon>
              <h3>Statistiques</h3>
              <p>Suivre vos performances de vente</p>
            </mat-card-content>
          </mat-card>
        </div>
      }
    </div>
  `,
  styles: [`
    .stats-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
      gap: 16px;
      margin-bottom: 32px;
    }

    .stat-card {
      mat-card-content {
        display: flex;
        align-items: center;
        gap: 16px;
        padding: 20px;
      }
    }

    .stat-icon {
      width: 56px;
      height: 56px;
      border-radius: 12px;
      display: flex;
      align-items: center;
      justify-content: center;

      mat-icon {
        font-size: 28px;
        width: 28px;
        height: 28px;
        color: white;
      }

      &.poulets { background: #2e7d32; }
      &.available { background: #1565c0; }
      &.sold { background: #7b1fa2; }
      &.revenue { background: #e65100; }
      &.orders { background: #00838f; }
      &.rating { background: #f9a825; }
    }

    .stat-info {
      display: flex;
      flex-direction: column;
    }

    .stat-value {
      font-size: 1.8rem;
      font-weight: 600;
      line-height: 1.2;
    }

    .stat-label {
      font-size: 0.85rem;
      color: var(--faso-text-secondary);
    }

    .section-divider {
      margin: 32px 0 16px;
    }

    .section-title {
      font-size: 1.4rem;
      color: var(--faso-primary-dark);
      margin-bottom: 16px;
    }

    .actions-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
      gap: 16px;
    }

    .action-card {
      cursor: pointer;
      transition: transform 0.2s, box-shadow 0.2s;

      &:hover {
        transform: translateY(-4px);
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
      }

      mat-card-content {
        text-align: center;
        padding: 32px 24px;
      }

      mat-icon {
        font-size: 48px;
        width: 48px;
        height: 48px;
        color: var(--faso-primary);
      }

      h3 {
        margin: 12px 0 8px;
      }

      p {
        color: var(--faso-text-secondary);
        font-size: 0.9rem;
        margin: 0;
      }
    }
  `],
})
export class EleveurDashboardComponent implements OnInit {
  private readonly pouletService = inject(PouletService);
  readonly auth = inject(AuthService);

  readonly stats = signal<EleveurStats | null>(null);
  readonly loading = signal(true);

  ngOnInit(): void {
    this.pouletService.getEleveurStats().subscribe({
      next: (data) => {
        this.stats.set(data);
        this.loading.set(false);
      },
      error: () => {
        this.loading.set(false);
      },
    });
  }
}
