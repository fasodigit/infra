import { Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { Router } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatDividerModule } from '@angular/material/divider';
import { MatListModule } from '@angular/material/list';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';

import { PanierService, PanierItem } from '@services/panier.service';
import { PouletService } from '@services/poulet.service';

@Component({
  selector: 'app-panier',
  standalone: true,
  imports: [
    CommonModule,
    ReactiveFormsModule,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatFormFieldModule,
    MatInputModule,
    MatDividerModule,
    MatListModule,
    MatProgressSpinnerModule,
    MatSnackBarModule,
  ],
  template: `
    <div class="container">
      <div class="page-header">
        <h1>Mon Panier</h1>
      </div>

      @if (panier.items().length === 0) {
        <div class="empty-cart">
          <mat-icon>shopping_cart</mat-icon>
          <h3>Votre panier est vide</h3>
          <p>Parcourez notre catalogue pour trouver des poulets frais.</p>
          <a mat-raised-button color="primary" routerLink="/client/catalogue">
            <mat-icon>storefront</mat-icon>
            Voir le catalogue
          </a>
        </div>
      } @else {
        <div class="panier-layout">
          <!-- Cart Items -->
          <div class="cart-items">
            @for (item of panier.items(); track item.poulet.id) {
              <mat-card class="cart-item">
                <div class="cart-item-content">
                  <div class="item-image">
                    @if (item.poulet.photos?.length) {
                      <img [src]="item.poulet.photos[0]" [alt]="item.poulet.race" />
                    } @else {
                      <div class="image-placeholder">
                        <mat-icon>egg_alt</mat-icon>
                      </div>
                    }
                  </div>

                  <div class="item-info">
                    <h3>{{ item.poulet.race }}</h3>
                    <p>{{ item.poulet.poids }} kg &mdash; {{ item.poulet.eleveur?.localisation }}</p>
                    <p class="item-unit-price">{{ item.poulet.prix | number:'1.0-0' }} FCFA / unite</p>
                  </div>

                  <div class="item-quantity">
                    <button mat-icon-button (click)="updateQuantity(item, item.quantite - 1)">
                      <mat-icon>remove</mat-icon>
                    </button>
                    <span class="quantity-value">{{ item.quantite }}</span>
                    <button mat-icon-button (click)="updateQuantity(item, item.quantite + 1)">
                      <mat-icon>add</mat-icon>
                    </button>
                  </div>

                  <div class="item-total">
                    <strong>{{ item.poulet.prix * item.quantite | number:'1.0-0' }} FCFA</strong>
                  </div>

                  <button mat-icon-button color="warn" (click)="removeItem(item.poulet.id)">
                    <mat-icon>delete</mat-icon>
                  </button>
                </div>
              </mat-card>
            }
          </div>

          <!-- Order Summary -->
          <mat-card class="order-summary">
            <mat-card-header>
              <mat-card-title>Recapitulatif</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="summary-row">
                <span>Articles ({{ panier.itemCount() }})</span>
                <span>{{ panier.total() | number:'1.0-0' }} FCFA</span>
              </div>
              <div class="summary-row">
                <span>Livraison</span>
                <span>Gratuite</span>
              </div>
              <mat-divider></mat-divider>
              <div class="summary-row total">
                <strong>Total</strong>
                <strong>{{ panier.total() | number:'1.0-0' }} FCFA</strong>
              </div>

              <form [formGroup]="orderForm" class="order-form">
                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>Adresse de livraison</mat-label>
                  <input matInput formControlName="adresse"
                         placeholder="Quartier, Secteur, Ville" />
                  <mat-icon matPrefix>location_on</mat-icon>
                </mat-form-field>

                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>Telephone</mat-label>
                  <input matInput formControlName="telephone"
                         placeholder="+226 70 00 00 00" />
                  <mat-icon matPrefix>phone</mat-icon>
                </mat-form-field>

                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>Notes (optionnel)</mat-label>
                  <textarea matInput formControlName="notes" rows="2"></textarea>
                </mat-form-field>
              </form>

              <button mat-raised-button color="primary" class="full-width checkout-btn"
                      [disabled]="orderForm.invalid || ordering()"
                      (click)="passerCommande()">
                @if (ordering()) {
                  <mat-spinner diameter="24"></mat-spinner>
                } @else {
                  <mat-icon>shopping_bag</mat-icon>
                  Commander
                }
              </button>
            </mat-card-content>
          </mat-card>
        </div>
      }
    </div>
  `,
  styles: [`
    .empty-cart {
      text-align: center;
      padding: 80px 24px;
      color: var(--faso-text-secondary);

      mat-icon {
        font-size: 80px;
        width: 80px;
        height: 80px;
        opacity: 0.3;
      }

      h3 {
        margin: 16px 0 8px;
        font-size: 1.4rem;
      }
    }

    .panier-layout {
      display: grid;
      grid-template-columns: 1fr 360px;
      gap: 24px;
      padding: 16px 0;

      @media (max-width: 768px) {
        grid-template-columns: 1fr;
      }
    }

    .cart-items {
      display: flex;
      flex-direction: column;
      gap: 12px;
    }

    .cart-item-content {
      display: flex;
      align-items: center;
      gap: 16px;
      padding: 12px;
    }

    .item-image {
      width: 80px;
      height: 80px;
      border-radius: 8px;
      overflow: hidden;
      flex-shrink: 0;

      img {
        width: 100%;
        height: 100%;
        object-fit: cover;
      }
    }

    .image-placeholder {
      display: flex;
      align-items: center;
      justify-content: center;
      width: 100%;
      height: 100%;
      background: #e8f5e9;
    }

    .item-info {
      flex: 1;

      h3 {
        margin: 0 0 4px;
        font-size: 1rem;
      }

      p {
        margin: 0;
        font-size: 0.85rem;
        color: var(--faso-text-secondary);
      }

      .item-unit-price {
        color: var(--faso-accent-dark);
        font-weight: 500;
      }
    }

    .item-quantity {
      display: flex;
      align-items: center;
      gap: 4px;

      .quantity-value {
        min-width: 30px;
        text-align: center;
        font-weight: 500;
        font-size: 1.1rem;
      }
    }

    .item-total {
      min-width: 100px;
      text-align: right;
      font-size: 1rem;
    }

    .order-summary {
      position: sticky;
      top: 80px;
      align-self: start;
      padding: 16px;
    }

    .summary-row {
      display: flex;
      justify-content: space-between;
      padding: 8px 0;
      font-size: 0.95rem;

      &.total {
        font-size: 1.2rem;
        padding: 16px 0;
      }
    }

    mat-divider {
      margin: 8px 0;
    }

    .order-form {
      margin-top: 16px;
    }

    .full-width {
      width: 100%;
    }

    .checkout-btn {
      height: 48px;
      font-size: 1rem;
      margin-top: 8px;
    }
  `],
})
export class PanierComponent {
  readonly panier = inject(PanierService);
  private readonly pouletService = inject(PouletService);
  private readonly router = inject(Router);
  private readonly snackBar = inject(MatSnackBar);
  private readonly fb = inject(FormBuilder);

  readonly ordering = signal(false);

  readonly orderForm = this.fb.nonNullable.group({
    adresse: ['', [Validators.required]],
    telephone: ['', [Validators.required]],
    notes: [''],
  });

  updateQuantity(item: PanierItem, qty: number): void {
    this.panier.mettreAJourQuantite(item.poulet.id, qty);
  }

  removeItem(pouletId: string): void {
    this.panier.retirer(pouletId);
  }

  passerCommande(): void {
    if (this.orderForm.invalid || this.panier.items().length === 0) return;

    this.ordering.set(true);
    const { adresse, telephone, notes } = this.orderForm.getRawValue();

    // Place orders for each item in the cart
    const items = this.panier.items();
    let completed = 0;
    let errors = 0;

    for (const item of items) {
      this.pouletService
        .passerCommande({
          pouletId: item.poulet.id,
          quantite: item.quantite,
          adresseLivraison: adresse,
          telephone,
          notes: notes || undefined,
        })
        .subscribe({
          next: () => {
            completed++;
            if (completed + errors === items.length) {
              this.finishOrder(completed, errors);
            }
          },
          error: () => {
            errors++;
            if (completed + errors === items.length) {
              this.finishOrder(completed, errors);
            }
          },
        });
    }
  }

  private finishOrder(completed: number, errors: number): void {
    this.ordering.set(false);

    if (errors === 0) {
      this.panier.vider();
      this.snackBar.open(
        `${completed} commande(s) passee(s) avec succes !`,
        'Voir',
        { duration: 5000, panelClass: 'snackbar-success' },
      );
      this.router.navigate(['/client/commandes']);
    } else {
      this.snackBar.open(
        `${completed} reussie(s), ${errors} erreur(s)`,
        'Fermer',
        { duration: 5000, panelClass: 'snackbar-error' },
      );
    }
  }
}
