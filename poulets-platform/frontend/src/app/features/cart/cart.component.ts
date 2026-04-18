// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, inject, computed } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

import { PanierService } from '@services/panier.service';
import { EmptyStateComponent } from '@shared/components/empty-state/empty-state.component';

@Component({
  selector: 'app-cart',
  standalone: true,
  imports: [CommonModule, RouterLink, DecimalPipe, MatIconModule, MatButtonModule, EmptyStateComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <div class="container">
        <header class="head">
          <h1>Mon panier</h1>
          <p>{{ panier.itemCount() }} article{{ panier.itemCount() > 1 ? 's' : '' }}</p>
        </header>

        @if (panier.items().length === 0) {
          <app-empty-state icon="shopping_basket" title="Votre panier est vide">
            <a mat-raised-button color="primary" routerLink="/marketplace/annonces">
              Parcourir les annonces
            </a>
          </app-empty-state>
        } @else {
          <div class="grid">
            <ul class="items">
              @for (item of panier.items(); track item.poulet.id) {
                <li>
                  <img
                    [src]="item.poulet.photos?.[0] || 'assets/img/placeholder-poulet.svg'"
                    [alt]="item.poulet.race"
                    loading="lazy"
                  >
                  <div class="meta">
                    <a [routerLink]="['/marketplace/annonces', item.poulet.id]">
                      <strong>{{ item.poulet.race }}</strong>
                    </a>
                    <p class="by">Par {{ item.poulet.eleveur?.nom || 'Éleveur' }}</p>
                    @if (item.poulet.description) {
                      <p class="desc">{{ item.poulet.description }}</p>
                    }
                  </div>

                  <div class="qty">
                    <button type="button" (click)="dec(item.poulet.id, item.quantite)" aria-label="Moins">−</button>
                    <span>{{ item.quantite }}</span>
                    <button type="button" (click)="inc(item.poulet.id, item.quantite)" aria-label="Plus">+</button>
                  </div>

                  <div class="lineprice">
                    <strong>{{ (item.poulet.prix * item.quantite) | number:'1.0-0' }} FCFA</strong>
                    <span>{{ item.poulet.prix | number:'1.0-0' }} × {{ item.quantite }}</span>
                  </div>

                  <button
                    type="button"
                    class="remove"
                    (click)="panier.retirer(item.poulet.id)"
                    aria-label="Retirer du panier"
                  >
                    <mat-icon>delete_outline</mat-icon>
                  </button>
                </li>
              }
            </ul>

            <aside class="summary">
              <h2>Récapitulatif</h2>
              <dl>
                <div><dt>Sous-total</dt><dd>{{ panier.total() | number:'1.0-0' }} FCFA</dd></div>
                <div><dt>Livraison (estimée)</dt><dd>{{ shipping() | number:'1.0-0' }} FCFA</dd></div>
                <div class="total"><dt>Total</dt><dd>{{ grandTotal() | number:'1.0-0' }} FCFA</dd></div>
              </dl>

              <a mat-raised-button color="primary" routerLink="/checkout" class="cta">
                <mat-icon>lock</mat-icon>
                Passer la commande
              </a>

              <button type="button" mat-button (click)="panier.vider()" class="clear">
                <mat-icon>delete_sweep</mat-icon>
                Vider le panier
              </button>

              <p class="trust">
                <mat-icon>verified_user</mat-icon>
                Paiement sécurisé · Livraison par éleveur vérifié
              </p>
            </aside>
          </div>
        }
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .container {
      max-width: 1200px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-12);
    }
    .head { margin-bottom: var(--faso-space-6); }
    .head h1 {
      margin: 0;
      font-size: var(--faso-text-3xl);
      font-weight: var(--faso-weight-bold);
    }
    .head p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .grid {
      display: grid;
      grid-template-columns: 1fr 320px;
      gap: var(--faso-space-6);
    }
    @media (max-width: 899px) {
      .grid { grid-template-columns: 1fr; }
    }

    .items { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--faso-space-3); }
    li {
      display: grid;
      grid-template-columns: 96px 1fr auto auto auto;
      gap: var(--faso-space-3);
      align-items: center;
      padding: var(--faso-space-3);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
    }
    li img {
      width: 96px;
      height: 96px;
      object-fit: cover;
      border-radius: var(--faso-radius-md);
    }
    .meta { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
    .meta a { color: var(--faso-text); text-decoration: none; }
    .meta strong { font-size: var(--faso-text-lg); }
    .by { margin: 0; color: var(--faso-text-muted); font-size: var(--faso-text-sm); }
    .desc {
      margin: 4px 0 0;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
      display: -webkit-box;
      -webkit-line-clamp: 2;
      -webkit-box-orient: vertical;
      overflow: hidden;
    }

    .qty {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      background: var(--faso-surface-alt);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-pill);
      padding: 2px;
    }
    .qty button {
      width: 28px;
      height: 28px;
      border: none;
      background: transparent;
      cursor: pointer;
      font-size: 18px;
      font-weight: 600;
      border-radius: 50%;
    }
    .qty button:hover { background: var(--faso-primary-50); }
    .qty span { min-width: 24px; text-align: center; font-weight: var(--faso-weight-medium); }

    .lineprice { display: flex; flex-direction: column; gap: 2px; text-align: right; }
    .lineprice strong { color: var(--faso-primary-700); font-size: var(--faso-text-lg); }
    .lineprice span { color: var(--faso-text-muted); font-size: var(--faso-text-xs); }

    .remove {
      background: transparent;
      border: none;
      cursor: pointer;
      padding: 6px;
      border-radius: 50%;
      color: var(--faso-danger);
    }
    .remove:hover { background: var(--faso-danger-bg); }

    .summary {
      position: sticky;
      top: var(--faso-space-6);
      align-self: flex-start;
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      box-shadow: var(--faso-shadow-sm);
    }
    .summary h2 {
      margin: 0 0 var(--faso-space-4);
      font-size: var(--faso-text-lg);
    }
    dl { margin: 0; display: flex; flex-direction: column; gap: 8px; }
    dl div {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
    }
    dl dt { color: var(--faso-text-muted); font-size: var(--faso-text-sm); }
    dl dd { margin: 0; font-weight: var(--faso-weight-medium); }
    dl .total {
      border-top: 1px solid var(--faso-border);
      padding-top: 12px;
      margin-top: 4px;
    }
    dl .total dd {
      color: var(--faso-primary-700);
      font-size: var(--faso-text-xl);
      font-weight: var(--faso-weight-bold);
    }

    .cta { margin-top: var(--faso-space-5); width: 100%; }
    .clear { margin-top: var(--faso-space-2); width: 100%; color: var(--faso-text-muted); }
    .trust {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      margin: var(--faso-space-4) 0 0;
      color: var(--faso-success);
      font-size: var(--faso-text-xs);
    }
    .trust mat-icon { font-size: 16px; width: 16px; height: 16px; }

    @media (max-width: 639px) {
      li {
        grid-template-columns: 80px 1fr auto;
        row-gap: var(--faso-space-2);
      }
      li img { width: 80px; height: 80px; }
      .qty, .lineprice { grid-column: 2; justify-self: start; text-align: left; }
      .remove { grid-column: 3; grid-row: 1; align-self: flex-start; }
    }
  `],
})
export class CartComponent {
  readonly panier = inject(PanierService);

  readonly shipping = computed(() => this.panier.total() > 50000 ? 0 : 2000);
  readonly grandTotal = computed(() => this.panier.total() + this.shipping());

  inc(id: string, q: number) { this.panier.mettreAJourQuantite(id, q + 1); }
  dec(id: string, q: number) { this.panier.mettreAJourQuantite(id, q - 1); }
}
