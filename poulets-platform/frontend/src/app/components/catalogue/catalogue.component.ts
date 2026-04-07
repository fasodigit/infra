import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ReactiveFormsModule, FormBuilder } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatChipsModule } from '@angular/material/chips';
import { MatPaginatorModule, PageEvent } from '@angular/material/paginator';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { MatDialogModule } from '@angular/material/dialog';

import { PouletService } from '@services/poulet.service';
import { PanierService } from '@services/panier.service';
import { AuthService } from '@services/auth.service';
import { Poulet, PouletFilter } from '@services/graphql.service';

@Component({
  selector: 'app-catalogue',
  standalone: true,
  imports: [
    CommonModule,
    ReactiveFormsModule,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    MatChipsModule,
    MatPaginatorModule,
    MatProgressSpinnerModule,
    MatSnackBarModule,
    MatDialogModule,
  ],
  template: `
    <div class="container">
      <div class="page-header">
        <h1>Catalogue des Poulets</h1>
        <p>Parcourez notre selection de poulets frais eleves au Burkina Faso</p>
      </div>

      <!-- Filters -->
      <div class="filters" [formGroup]="filterForm">
        <mat-form-field appearance="outline">
          <mat-label>Race</mat-label>
          <mat-select formControlName="race">
            <mat-option value="">Toutes</mat-option>
            <mat-option value="bicyclette">Bicyclette</mat-option>
            <mat-option value="coucou">Coucou de Malines</mat-option>
            <mat-option value="brahma">Brahma</mat-option>
            <mat-option value="sussex">Sussex</mat-option>
            <mat-option value="locale">Race Locale</mat-option>
          </mat-select>
        </mat-form-field>

        <mat-form-field appearance="outline">
          <mat-label>Prix min (FCFA)</mat-label>
          <input matInput type="number" formControlName="prixMin" />
        </mat-form-field>

        <mat-form-field appearance="outline">
          <mat-label>Prix max (FCFA)</mat-label>
          <input matInput type="number" formControlName="prixMax" />
        </mat-form-field>

        <mat-form-field appearance="outline">
          <mat-label>Localisation</mat-label>
          <mat-select formControlName="localisation">
            <mat-option value="">Toutes</mat-option>
            <mat-option value="ouagadougou">Ouagadougou</mat-option>
            <mat-option value="bobo-dioulasso">Bobo-Dioulasso</mat-option>
            <mat-option value="koudougou">Koudougou</mat-option>
            <mat-option value="ouahigouya">Ouahigouya</mat-option>
          </mat-select>
        </mat-form-field>

        <button mat-raised-button color="primary" (click)="applyFilters()">
          <mat-icon>search</mat-icon>
          Rechercher
        </button>

        <button mat-button (click)="resetFilters()">
          <mat-icon>clear</mat-icon>
          Reinitialiser
        </button>
      </div>

      <!-- Results -->
      @if (loading()) {
        <div class="loading-overlay">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else {
        <div class="results-header">
          <span>{{ totalElements() }} poulet(s) trouve(s)</span>
        </div>

        <div class="card-grid">
          @for (poulet of poulets(); track poulet.id) {
            <mat-card class="poulet-card">
              <div class="poulet-image">
                @if (poulet.photos?.length) {
                  <img [src]="poulet.photos[0]" [alt]="poulet.race" />
                } @else {
                  <div class="poulet-image-placeholder">
                    <mat-icon>egg_alt</mat-icon>
                  </div>
                }
                <mat-chip class="status-chip"
                          [class.available]="poulet.statut === 'DISPONIBLE'">
                  {{ poulet.statut }}
                </mat-chip>
              </div>

              <mat-card-header>
                <mat-card-title>{{ poulet.race }}</mat-card-title>
                <mat-card-subtitle>
                  <mat-icon inline>location_on</mat-icon>
                  {{ poulet.eleveur?.localisation }}
                  &mdash;
                  <mat-icon inline>star</mat-icon>
                  {{ poulet.eleveur?.note | number:'1.1-1' }}
                </mat-card-subtitle>
              </mat-card-header>

              <mat-card-content>
                <p class="poulet-description">{{ poulet.description }}</p>
                <div class="poulet-details">
                  <span><mat-icon inline>scale</mat-icon> {{ poulet.poids }} kg</span>
                  <span><mat-icon inline>cake</mat-icon> {{ poulet.age }} sem.</span>
                </div>
                <p class="poulet-price">{{ poulet.prix | number:'1.0-0' }} FCFA</p>
              </mat-card-content>

              <mat-card-actions>
                @if (poulet.statut === 'DISPONIBLE' && auth.isAuthenticated()) {
                  <button mat-raised-button color="accent"
                          (click)="ajouterAuPanier(poulet)">
                    <mat-icon>add_shopping_cart</mat-icon>
                    Ajouter au panier
                  </button>
                } @else if (!auth.isAuthenticated()) {
                  <button mat-stroked-button color="primary" routerLink="/login">
                    Se connecter pour acheter
                  </button>
                } @else {
                  <button mat-button disabled>
                    Non disponible
                  </button>
                }
              </mat-card-actions>
            </mat-card>
          } @empty {
            <div class="empty-state">
              <mat-icon>search_off</mat-icon>
              <p>Aucun poulet ne correspond a vos criteres.</p>
              <button mat-button color="primary" (click)="resetFilters()">
                Reinitialiser les filtres
              </button>
            </div>
          }
        </div>

        @if (totalElements() > 0) {
          <mat-paginator
            [length]="totalElements()"
            [pageSize]="pageSize"
            [pageSizeOptions]="[12, 24, 48]"
            [pageIndex]="currentPage()"
            (page)="onPageChange($event)"
            showFirstLastButtons>
          </mat-paginator>
        }
      }
    </div>
  `,
  styles: [`
    .filters {
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
      align-items: center;
      padding: 16px;
      background: var(--faso-surface);
      border-radius: 8px;
      margin-bottom: 24px;
      box-shadow: 0 1px 4px rgba(0, 0, 0, 0.06);

      mat-form-field {
        flex: 1;
        min-width: 150px;
      }
    }

    .results-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 16px;
      color: var(--faso-text-secondary);
      font-size: 0.9rem;
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
      height: 200px;
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

    .status-chip.available {
      background: var(--faso-primary);
      color: white;
    }

    .poulet-description {
      font-size: 0.9rem;
      color: var(--faso-text-secondary);
      overflow: hidden;
      text-overflow: ellipsis;
      display: -webkit-box;
      -webkit-line-clamp: 2;
      -webkit-box-orient: vertical;
    }

    .poulet-details {
      display: flex;
      gap: 16px;
      margin: 8px 0;
      color: var(--faso-text-secondary);
      font-size: 0.85rem;

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

    mat-paginator {
      margin-top: 24px;
    }
  `],
})
export class CatalogueComponent implements OnInit {
  private readonly pouletService = inject(PouletService);
  private readonly fb = inject(FormBuilder);
  private readonly snackBar = inject(MatSnackBar);
  readonly auth = inject(AuthService);
  readonly panier = inject(PanierService);

  readonly poulets = signal<Poulet[]>([]);
  readonly loading = signal(true);
  readonly totalElements = signal(0);
  readonly currentPage = signal(0);
  readonly pageSize = 12;

  readonly filterForm = this.fb.nonNullable.group({
    race: [''],
    prixMin: [null as number | null],
    prixMax: [null as number | null],
    localisation: [''],
  });

  ngOnInit(): void {
    this.loadPoulets();
  }

  applyFilters(): void {
    this.currentPage.set(0);
    this.loadPoulets();
  }

  resetFilters(): void {
    this.filterForm.reset();
    this.currentPage.set(0);
    this.loadPoulets();
  }

  onPageChange(event: PageEvent): void {
    this.currentPage.set(event.pageIndex);
    this.loadPoulets();
  }

  ajouterAuPanier(poulet: Poulet): void {
    this.panier.ajouter(poulet);
    this.snackBar.open(`${poulet.race} ajoute au panier`, 'Voir', {
      duration: 3000,
      panelClass: 'snackbar-success',
    });
  }

  private loadPoulets(): void {
    this.loading.set(true);
    const values = this.filterForm.getRawValue();

    const filter: PouletFilter = {};
    if (values.race) filter.race = values.race;
    if (values.prixMin) filter.prixMin = values.prixMin;
    if (values.prixMax) filter.prixMax = values.prixMax;
    if (values.localisation) filter.localisation = values.localisation;
    filter.statut = 'DISPONIBLE';

    this.pouletService
      .getPoulets(filter, this.currentPage(), this.pageSize)
      .subscribe({
        next: (page) => {
          this.poulets.set(page.content);
          this.totalElements.set(page.totalElements);
          this.loading.set(false);
        },
        error: () => {
          this.loading.set(false);
          this.snackBar.open('Erreur lors du chargement', 'Fermer', {
            duration: 3000,
            panelClass: 'snackbar-error',
          });
        },
      });
  }
}
