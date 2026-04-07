import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { MatChipsModule } from '@angular/material/chips';
import { MatDialogModule, MatDialog } from '@angular/material/dialog';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { MatPaginatorModule, PageEvent } from '@angular/material/paginator';

import { PouletService } from '@services/poulet.service';
import { Poulet, CreatePouletInput } from '@services/graphql.service';

@Component({
  selector: 'app-eleveur-poulets',
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
    MatTableModule,
    MatChipsModule,
    MatDialogModule,
    MatProgressSpinnerModule,
    MatSnackBarModule,
    MatPaginatorModule,
  ],
  template: `
    <div class="container">
      <div class="page-header">
        <h1>Mes Poulets</h1>
        <button mat-raised-button color="primary" (click)="showForm.set(!showForm())">
          <mat-icon>{{ showForm() ? 'close' : 'add' }}</mat-icon>
          {{ showForm() ? 'Annuler' : 'Ajouter un poulet' }}
        </button>
      </div>

      <!-- Add Poulet Form -->
      @if (showForm()) {
        <mat-card class="add-form-card">
          <mat-card-header>
            <mat-card-title>Nouveau poulet</mat-card-title>
          </mat-card-header>
          <mat-card-content>
            <form [formGroup]="pouletForm" (ngSubmit)="onAddPoulet()" class="poulet-form">
              <div class="form-row">
                <mat-form-field appearance="outline">
                  <mat-label>Race</mat-label>
                  <mat-select formControlName="race">
                    <mat-option value="bicyclette">Bicyclette</mat-option>
                    <mat-option value="coucou">Coucou de Malines</mat-option>
                    <mat-option value="brahma">Brahma</mat-option>
                    <mat-option value="sussex">Sussex</mat-option>
                    <mat-option value="locale">Race Locale</mat-option>
                  </mat-select>
                </mat-form-field>

                <mat-form-field appearance="outline">
                  <mat-label>Age (semaines)</mat-label>
                  <input matInput type="number" formControlName="age" />
                </mat-form-field>

                <mat-form-field appearance="outline">
                  <mat-label>Poids (kg)</mat-label>
                  <input matInput type="number" step="0.1" formControlName="poids" />
                </mat-form-field>

                <mat-form-field appearance="outline">
                  <mat-label>Prix (FCFA)</mat-label>
                  <input matInput type="number" formControlName="prix" />
                </mat-form-field>
              </div>

              <mat-form-field appearance="outline" class="full-width">
                <mat-label>Description</mat-label>
                <textarea matInput formControlName="description" rows="3"
                          placeholder="Decrivez votre poulet (alimentation, conditions d'elevage...)">
                </textarea>
              </mat-form-field>

              <mat-form-field appearance="outline" class="full-width">
                <mat-label>Alimentation</mat-label>
                <input matInput formControlName="alimentation"
                       placeholder="Ex: grains bio, mais local, restes de cuisine" />
              </mat-form-field>

              <div class="form-actions">
                <button mat-raised-button color="primary" type="submit"
                        [disabled]="pouletForm.invalid || addingPoulet()">
                  @if (addingPoulet()) {
                    <mat-spinner diameter="24"></mat-spinner>
                  } @else {
                    <mat-icon>save</mat-icon>
                    Enregistrer
                  }
                </button>
                <button mat-button type="button" (click)="showForm.set(false)">
                  Annuler
                </button>
              </div>
            </form>
          </mat-card-content>
        </mat-card>
      }

      <!-- Poulets List -->
      @if (loading()) {
        <div class="loading-overlay">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else {
        <div class="card-grid">
          @for (poulet of mesPoulets(); track poulet.id) {
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
                          [class.disponible]="poulet.statut === 'DISPONIBLE'"
                          [class.vendu]="poulet.statut === 'VENDU'"
                          [class.reserve]="poulet.statut === 'RESERVE'">
                  {{ poulet.statut }}
                </mat-chip>
              </div>

              <mat-card-header>
                <mat-card-title>{{ poulet.race }}</mat-card-title>
                <mat-card-subtitle>
                  Ajoute le {{ poulet.createdAt | date:'dd/MM/yyyy' }}
                </mat-card-subtitle>
              </mat-card-header>

              <mat-card-content>
                <div class="poulet-details">
                  <span><mat-icon inline>scale</mat-icon> {{ poulet.poids }} kg</span>
                  <span><mat-icon inline>cake</mat-icon> {{ poulet.age }} sem.</span>
                </div>
                <p class="poulet-price">{{ poulet.prix | number:'1.0-0' }} FCFA</p>
              </mat-card-content>

              <mat-card-actions>
                @if (poulet.statut === 'DISPONIBLE') {
                  <button mat-button color="warn" (click)="deletePoulet(poulet.id)">
                    <mat-icon>delete</mat-icon>
                    Retirer
                  </button>
                }
              </mat-card-actions>
            </mat-card>
          } @empty {
            <div class="empty-state">
              <mat-icon>egg_alt</mat-icon>
              <p>Vous n'avez aucun poulet enregistre.</p>
              <button mat-raised-button color="primary" (click)="showForm.set(true)">
                Ajouter votre premier poulet
              </button>
            </div>
          }
        </div>

        @if (totalElements() > 0) {
          <mat-paginator
            [length]="totalElements()"
            [pageSize]="20"
            [pageIndex]="currentPage()"
            (page)="onPageChange($event)"
            showFirstLastButtons>
          </mat-paginator>
        }
      }
    </div>
  `,
  styles: [`
    .page-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    .add-form-card {
      margin-bottom: 24px;
      padding: 16px;
    }

    .poulet-form {
      padding: 16px 0;
    }

    .form-row {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 12px;
    }

    .full-width {
      width: 100%;
    }

    .form-actions {
      display: flex;
      gap: 12px;
      margin-top: 16px;
    }

    .poulet-card {
      transition: transform 0.2s;

      &:hover {
        transform: translateY(-2px);
      }
    }

    .poulet-image {
      position: relative;
      height: 160px;
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
        font-size: 48px;
        width: 48px;
        height: 48px;
        color: var(--faso-primary-light);
      }
    }

    .status-chip {
      position: absolute;
      top: 8px;
      right: 8px;

      &.disponible { background: var(--faso-primary); color: white; }
      &.vendu { background: #7b1fa2; color: white; }
      &.reserve { background: var(--faso-accent); color: white; }
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
      font-size: 1.3rem;
      font-weight: 600;
      color: var(--faso-accent-dark);
      margin: 4px 0 0;
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
export class EleveurPouletsComponent implements OnInit {
  private readonly pouletService = inject(PouletService);
  private readonly fb = inject(FormBuilder);
  private readonly snackBar = inject(MatSnackBar);

  readonly mesPoulets = signal<Poulet[]>([]);
  readonly loading = signal(true);
  readonly showForm = signal(false);
  readonly addingPoulet = signal(false);
  readonly totalElements = signal(0);
  readonly currentPage = signal(0);

  readonly pouletForm = this.fb.nonNullable.group({
    race: ['', [Validators.required]],
    age: [0, [Validators.required, Validators.min(1)]],
    poids: [0, [Validators.required, Validators.min(0.1)]],
    prix: [0, [Validators.required, Validators.min(100)]],
    description: ['', [Validators.required, Validators.minLength(10)]],
    alimentation: [''],
  });

  ngOnInit(): void {
    this.loadMesPoulets();
  }

  onAddPoulet(): void {
    if (this.pouletForm.invalid) return;

    this.addingPoulet.set(true);
    const values = this.pouletForm.getRawValue();

    const input: CreatePouletInput = {
      race: values.race,
      age: values.age,
      poids: values.poids,
      prix: values.prix,
      description: values.description,
      alimentation: values.alimentation || undefined,
    };

    this.pouletService.createPoulet(input).subscribe({
      next: () => {
        this.addingPoulet.set(false);
        this.showForm.set(false);
        this.pouletForm.reset();
        this.loadMesPoulets();
        this.snackBar.open('Poulet ajoute avec succes !', 'Fermer', {
          duration: 3000,
          panelClass: 'snackbar-success',
        });
      },
      error: () => {
        this.addingPoulet.set(false);
        this.snackBar.open('Erreur lors de l\'ajout', 'Fermer', {
          duration: 3000,
          panelClass: 'snackbar-error',
        });
      },
    });
  }

  deletePoulet(id: string): void {
    this.pouletService.deletePoulet(id).subscribe({
      next: () => {
        this.loadMesPoulets();
        this.snackBar.open('Poulet retire du catalogue', 'Fermer', {
          duration: 3000,
        });
      },
      error: () => {
        this.snackBar.open('Erreur lors de la suppression', 'Fermer', {
          duration: 3000,
          panelClass: 'snackbar-error',
        });
      },
    });
  }

  onPageChange(event: PageEvent): void {
    this.currentPage.set(event.pageIndex);
    this.loadMesPoulets();
  }

  private loadMesPoulets(): void {
    this.loading.set(true);
    this.pouletService.getMesPoulets(this.currentPage(), 20).subscribe({
      next: (page) => {
        this.mesPoulets.set(page.content);
        this.totalElements.set(page.totalElements);
        this.loading.set(false);
      },
      error: () => {
        this.loading.set(false);
      },
    });
  }
}
