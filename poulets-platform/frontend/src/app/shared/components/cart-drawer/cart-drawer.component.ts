// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { PanierService } from '@services/panier.service';

@Component({
  selector: 'app-cart-drawer',
  standalone: true,
  imports: [CommonModule, RouterLink, MatIconModule, MatButtonModule, DecimalPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <button
      type="button"
      class="trigger"
      (click)="open.set(true)"
      [attr.aria-label]="'Ouvrir panier'"
      [attr.aria-expanded]="open()"
    >
      <mat-icon>shopping_cart</mat-icon>
      @if (panier.itemCount() > 0) {
        <span class="badge">{{ panier.itemCount() }}</span>
      }
    </button>

    @if (open()) {
      <div class="backdrop" (click)="open.set(false)" aria-hidden="true"></div>
      <aside class="drawer" role="dialog" aria-label="Panier">
        <header>
          <h2>Votre panier · {{ panier.itemCount() }}</h2>
          <button type="button" class="close" (click)="open.set(false)" aria-label="Fermer">
            <mat-icon>close</mat-icon>
          </button>
        </header>

        <div class="body">
          @if (panier.items().length === 0) {
            <div class="empty">
              <mat-icon>shopping_basket</mat-icon>
              <p>Votre panier est vide</p>
              <a mat-stroked-button routerLink="/marketplace/annonces" (click)="open.set(false)">
                Parcourir les annonces
              </a>
            </div>
          } @else {
            <ul>
              @for (item of panier.items(); track item.poulet.id) {
                <li>
                  <img
                    [src]="item.poulet.photos?.[0] || 'assets/img/placeholder-poulet.svg'"
                    [alt]="item.poulet.race"
                    loading="lazy"
                  >
                  <div class="meta">
                    <strong>{{ item.poulet.race }}</strong>
                    <span class="sub">{{ item.poulet.eleveur.nom }}</span>
                    <span class="price">{{ item.poulet.prix | number:'1.0-0' }} FCFA</span>
                  </div>
                  <div class="qty">
                    <button type="button" (click)="dec(item.poulet.id, item.quantite)" aria-label="Moins">−</button>
                    <span>{{ item.quantite }}</span>
                    <button type="button" (click)="inc(item.poulet.id, item.quantite)" aria-label="Plus">+</button>
                  </div>
                  <button type="button" class="remove" (click)="panier.retirer(item.poulet.id)" aria-label="Retirer">
                    <mat-icon>delete_outline</mat-icon>
                  </button>
                </li>
              }
            </ul>
          }
        </div>

        @if (panier.items().length > 0) {
          <footer>
            <div class="total">
              <span>Total</span>
              <strong>{{ panier.total() | number:'1.0-0' }} FCFA</strong>
            </div>
            <div class="actions">
              <a mat-stroked-button routerLink="/cart" (click)="open.set(false)">Voir le panier</a>
              <a mat-raised-button color="primary" routerLink="/checkout" (click)="open.set(false)">
                Commander
              </a>
            </div>
          </footer>
        }
      </aside>
    }
  `,
  styles: [`
    :host { display: inline-flex; }

    .trigger {
      position: relative;
      background: transparent;
      border: none;
      color: inherit;
      cursor: pointer;
      padding: 8px;
      border-radius: 50%;
      line-height: 0;
    }
    .trigger:hover { background: var(--faso-surface-alt); }
    .badge {
      position: absolute;
      top: 0;
      right: 0;
      background: var(--faso-danger);
      color: #FFFFFF;
      border-radius: var(--faso-radius-pill);
      font-size: 10px;
      font-weight: 700;
      padding: 1px 5px;
      min-width: 16px;
      text-align: center;
    }

    .backdrop {
      position: fixed; inset: 0;
      background: var(--faso-overlay);
      z-index: var(--faso-z-drawer);
    }
    .drawer {
      position: fixed;
      top: 0; right: 0;
      height: 100dvh;
      width: min(420px, 100vw);
      background: var(--faso-surface);
      box-shadow: var(--faso-shadow-xl);
      z-index: calc(var(--faso-z-drawer) + 1);
      display: flex;
      flex-direction: column;
      animation: slide 240ms cubic-bezier(0, 0, 0.2, 1);
    }
    @keyframes slide {
      from { transform: translateX(100%); }
      to   { transform: translateX(0); }
    }
    .drawer header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: var(--faso-space-4) var(--faso-space-5);
      border-bottom: 1px solid var(--faso-border);
    }
    .drawer header h2 { margin: 0; font-size: var(--faso-text-xl); }
    .close {
      background: transparent;
      border: none;
      padding: 4px;
      border-radius: var(--faso-radius-md);
      cursor: pointer;
      color: var(--faso-text-muted);
    }
    .close:hover { background: var(--faso-surface-alt); }

    .drawer .body {
      flex: 1;
      overflow-y: auto;
      padding: var(--faso-space-4);
    }

    .empty {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: var(--faso-space-3);
      padding: var(--faso-space-10) var(--faso-space-4);
      color: var(--faso-text-muted);
      text-align: center;
    }
    .empty mat-icon { font-size: 48px; width: 48px; height: 48px; color: var(--faso-text-subtle); }

    ul { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--faso-space-3); }
    li {
      display: grid;
      grid-template-columns: 64px 1fr auto auto;
      gap: var(--faso-space-3);
      padding: var(--faso-space-2);
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-md);
      align-items: center;
    }
    li img {
      width: 64px;
      height: 64px;
      object-fit: cover;
      border-radius: var(--faso-radius-sm);
    }
    .meta { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
    .meta strong {
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-semibold);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .meta .sub {
      color: var(--faso-text-muted);
      font-size: var(--faso-text-xs);
    }
    .meta .price {
      color: var(--faso-primary-700);
      font-weight: var(--faso-weight-semibold);
      font-size: var(--faso-text-sm);
    }
    .qty {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-pill);
      padding: 2px;
    }
    .qty button {
      width: 24px;
      height: 24px;
      border: none;
      background: transparent;
      cursor: pointer;
      font-size: 16px;
      font-weight: 600;
      color: var(--faso-text);
      border-radius: 50%;
    }
    .qty button:hover { background: var(--faso-primary-50); }
    .qty span { min-width: 20px; text-align: center; font-size: var(--faso-text-sm); }

    .remove {
      background: transparent;
      border: none;
      cursor: pointer;
      padding: 4px;
      color: var(--faso-danger);
      border-radius: 50%;
    }
    .remove:hover { background: var(--faso-danger-bg); }

    footer {
      padding: var(--faso-space-4) var(--faso-space-5);
      border-top: 1px solid var(--faso-border);
      background: var(--faso-surface-alt);
    }
    .total {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      margin-bottom: var(--faso-space-3);
    }
    .total strong {
      font-size: var(--faso-text-xl);
      color: var(--faso-primary-700);
    }
    .actions {
      display: flex;
      gap: var(--faso-space-2);
    }
    .actions a { flex: 1; }
  `],
})
export class CartDrawerComponent {
  readonly panier = inject(PanierService);
  readonly open = signal(false);

  inc(id: string, q: number) { this.panier.mettreAJourQuantite(id, q + 1); }
  dec(id: string, q: number) { this.panier.mettreAJourQuantite(id, q - 1); }
}
