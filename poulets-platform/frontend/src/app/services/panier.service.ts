import { Injectable, signal, computed } from '@angular/core';
import { Poulet } from './graphql.service';

export interface PanierItem {
  poulet: Poulet;
  quantite: number;
}

@Injectable({ providedIn: 'root' })
export class PanierService {
  private readonly _items = signal<PanierItem[]>([]);

  readonly items = this._items.asReadonly();

  readonly itemCount = computed(() =>
    this._items().reduce((sum, item) => sum + item.quantite, 0),
  );

  readonly total = computed(() =>
    this._items().reduce((sum, item) => sum + item.poulet.prix * item.quantite, 0),
  );

  /**
   * Add a poulet to the cart. If already present, increment quantity.
   */
  ajouter(poulet: Poulet, quantite: number = 1): void {
    const current = this._items();
    const existing = current.find((item) => item.poulet.id === poulet.id);

    if (existing) {
      this._items.set(
        current.map((item) =>
          item.poulet.id === poulet.id
            ? { ...item, quantite: item.quantite + quantite }
            : item,
        ),
      );
    } else {
      this._items.set([...current, { poulet, quantite }]);
    }

    this.persister();
  }

  /**
   * Remove a poulet from the cart entirely.
   */
  retirer(pouletId: string): void {
    this._items.set(this._items().filter((item) => item.poulet.id !== pouletId));
    this.persister();
  }

  /**
   * Update the quantity for a specific item.
   */
  mettreAJourQuantite(pouletId: string, quantite: number): void {
    if (quantite <= 0) {
      this.retirer(pouletId);
      return;
    }

    this._items.set(
      this._items().map((item) =>
        item.poulet.id === pouletId ? { ...item, quantite } : item,
      ),
    );
    this.persister();
  }

  /**
   * Empty the cart.
   */
  vider(): void {
    this._items.set([]);
    this.persister();
  }

  /**
   * Restore cart from localStorage on app init.
   */
  restaurer(): void {
    try {
      const saved = localStorage.getItem('faso_panier');
      if (saved) {
        const parsed = JSON.parse(saved) as PanierItem[];
        this._items.set(parsed);
      }
    } catch {
      // Silently ignore corrupted data
      localStorage.removeItem('faso_panier');
    }
  }

  private persister(): void {
    try {
      localStorage.setItem('faso_panier', JSON.stringify(this._items()));
    } catch {
      // Storage full or disabled
    }
  }
}
