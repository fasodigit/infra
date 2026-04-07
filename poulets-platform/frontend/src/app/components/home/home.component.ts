import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';

import { PouletService } from '@services/poulet.service';
import { Poulet } from '@services/graphql.service';

@Component({
  selector: 'app-home',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    MatProgressSpinnerModule,
  ],
  template: `
    <!-- Hero Section -->
    <section class="hero">
      <div class="container">
        <h1>Poulets Frais du Burkina Faso</h1>
        <p>
          Plateforme de vente directe entre eleveurs et consommateurs.
          Des poulets de qualite, eleves localement, livres chez vous.
        </p>
        <div class="hero-actions">
          <a mat-raised-button color="accent" routerLink="/client/catalogue" class="hero-btn">
            <mat-icon>storefront</mat-icon>
            Voir le Catalogue
          </a>
          <a mat-stroked-button routerLink="/register" class="hero-btn hero-btn-outline">
            <mat-icon>person_add</mat-icon>
            Devenir Eleveur
          </a>
        </div>
      </div>
    </section>

    <!-- Features Section -->
    <section class="features container">
      <h2>Comment ca marche ?</h2>
      <div class="feature-grid">
        <div class="feature-card">
          <mat-icon class="feature-icon">search</mat-icon>
          <h3>Parcourir</h3>
          <p>Explorez notre catalogue de poulets frais de differentes races et tailles.</p>
        </div>
        <div class="feature-card">
          <mat-icon class="feature-icon">shopping_cart</mat-icon>
          <h3>Commander</h3>
          <p>Selectionnez vos poulets et passez commande directement aupres de l'eleveur.</p>
        </div>
        <div class="feature-card">
          <mat-icon class="feature-icon">local_shipping</mat-icon>
          <h3>Recevoir</h3>
          <p>Recevez vos poulets frais livres directement a votre porte.</p>
        </div>
      </div>
    </section>

    <!-- Latest Poulets Section -->
    <section class="latest container">
      <div class="section-header">
        <h2>Poulets Recents</h2>
        <a mat-button color="primary" routerLink="/client/catalogue">
          Voir tout <mat-icon>arrow_forward</mat-icon>
        </a>
      </div>

      @if (loading()) {
        <div class="loading-overlay">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else {
        <div class="card-grid">
          @for (poulet of latestPoulets(); track poulet.id) {
            <mat-card class="poulet-card">
              <div class="poulet-image">
                @if (poulet.photos?.length) {
                  <img [src]="poulet.photos[0]" [alt]="poulet.race" />
                } @else {
                  <div class="poulet-image-placeholder">
                    <mat-icon>egg_alt</mat-icon>
                  </div>
                }
                <mat-chip class="status-chip">{{ poulet.statut }}</mat-chip>
              </div>
              <mat-card-header>
                <mat-card-title>{{ poulet.race }}</mat-card-title>
                <mat-card-subtitle>
                  {{ poulet.eleveur?.localisation }}
                </mat-card-subtitle>
              </mat-card-header>
              <mat-card-content>
                <div class="poulet-details">
                  <span><mat-icon inline>scale</mat-icon> {{ poulet.poids }} kg</span>
                  <span><mat-icon inline>cake</mat-icon> {{ poulet.age }} semaines</span>
                </div>
                <p class="poulet-price">{{ poulet.prix | number:'1.0-0' }} FCFA</p>
              </mat-card-content>
              <mat-card-actions>
                <a mat-button color="primary" [routerLink]="['/client/catalogue']"
                   [queryParams]="{id: poulet.id}">
                  Voir details
                </a>
              </mat-card-actions>
            </mat-card>
          } @empty {
            <div class="empty-state">
              <mat-icon>egg_alt</mat-icon>
              <p>Aucun poulet disponible pour le moment.</p>
            </div>
          }
        </div>
      }
    </section>
  `,
  styles: [`
    .hero {
      background: linear-gradient(135deg, var(--faso-primary) 0%, var(--faso-primary-dark) 100%);
      color: white;
      padding: 80px 0;
      text-align: center;

      h1 {
        font-size: 2.5rem;
        margin: 0 0 16px;
        font-weight: 600;
      }

      p {
        font-size: 1.15rem;
        max-width: 600px;
        margin: 0 auto 32px;
        opacity: 0.9;
        line-height: 1.6;
      }
    }

    .hero-actions {
      display: flex;
      gap: 16px;
      justify-content: center;
      flex-wrap: wrap;
    }

    .hero-btn {
      padding: 8px 24px;
      font-size: 1rem;
    }

    .hero-btn-outline {
      border-color: white;
      color: white;
    }

    .features {
      padding: 64px 16px;
      text-align: center;

      h2 {
        font-size: 1.8rem;
        margin-bottom: 40px;
        color: var(--faso-primary-dark);
      }
    }

    .feature-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
      gap: 32px;
    }

    .feature-card {
      padding: 24px;
      border-radius: 12px;
      background: var(--faso-surface);
      box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);

      h3 {
        margin: 12px 0 8px;
        font-size: 1.2rem;
      }

      p {
        color: var(--faso-text-secondary);
        line-height: 1.5;
      }
    }

    .feature-icon {
      font-size: 48px;
      width: 48px;
      height: 48px;
      color: var(--faso-primary);
    }

    .latest {
      padding: 48px 16px 64px;
    }

    .section-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 24px;

      h2 {
        font-size: 1.6rem;
        color: var(--faso-primary-dark);
        margin: 0;
      }
    }

    .poulet-card {
      transition: transform 0.2s, box-shadow 0.2s;

      &:hover {
        transform: translateY(-4px);
        box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
      }
    }

    .poulet-image {
      position: relative;
      height: 180px;
      overflow: hidden;
      border-radius: 4px 4px 0 0;

      img {
        width: 100%;
        height: 100%;
        object-fit: cover;
      }
    }

    .poulet-image-placeholder {
      display: flex;
      align-items: center;
      justify-content: center;
      height: 100%;
      background: #e8f5e9;

      mat-icon {
        font-size: 64px;
        width: 64px;
        height: 64px;
        color: var(--faso-primary-light);
      }
    }

    .status-chip {
      position: absolute;
      top: 8px;
      right: 8px;
    }

    .poulet-details {
      display: flex;
      gap: 16px;
      margin: 8px 0;
      color: var(--faso-text-secondary);
      font-size: 0.9rem;

      span {
        display: flex;
        align-items: center;
        gap: 4px;
      }
    }

    .poulet-price {
      font-size: 1.4rem;
      font-weight: 600;
      color: var(--faso-accent-dark);
      margin: 8px 0 0;
    }

    .empty-state {
      text-align: center;
      padding: 48px;
      color: var(--faso-text-secondary);
      grid-column: 1 / -1;

      mat-icon {
        font-size: 64px;
        width: 64px;
        height: 64px;
        opacity: 0.4;
      }
    }
  `],
})
export class HomeComponent implements OnInit {
  private readonly pouletService = inject(PouletService);

  readonly latestPoulets = signal<Poulet[]>([]);
  readonly loading = signal(true);

  ngOnInit(): void {
    this.pouletService.getPoulets(undefined, 0, 6).subscribe({
      next: (page) => {
        this.latestPoulets.set(page.content);
        this.loading.set(false);
      },
      error: () => {
        this.loading.set(false);
      },
    });
  }
}
