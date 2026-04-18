import { ChangeDetectionStrategy, Component, EventEmitter, Input, Output } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

@Component({
  selector: 'app-filter-drawer',
  standalone: true,
  imports: [CommonModule, MatIconModule, MatButtonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <button
      type="button"
      class="trigger"
      (click)="open = true"
      [attr.aria-expanded]="open"
    >
      <mat-icon>tune</mat-icon>
      Filtres
      @if (activeCount > 0) { <span class="count">{{ activeCount }}</span> }
    </button>

    @if (open) {
      <div class="backdrop" (click)="open = false" aria-hidden="true"></div>
      <aside class="drawer" role="dialog" aria-label="Filtres">
        <header>
          <h2>Filtres</h2>
          <button type="button" class="close" (click)="open = false" aria-label="Fermer">
            <mat-icon>close</mat-icon>
          </button>
        </header>
        <div class="body"><ng-content></ng-content></div>
        <footer>
          <button mat-button type="button" (click)="clear()">Tout effacer</button>
          <button mat-raised-button color="primary" type="button" (click)="apply()">
            Voir les résultats
          </button>
        </footer>
      </aside>
    }
  `,
  styles: [`
    :host { display: inline-flex; }

    .trigger {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 8px 16px;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-pill);
      font-weight: var(--faso-weight-semibold);
      cursor: pointer;
      color: var(--faso-text);
      transition: border-color var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .trigger:hover { border-color: var(--faso-primary-500); }
    .trigger .count {
      background: var(--faso-primary-600);
      color: var(--faso-text-inverse);
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      padding: 1px 8px;
      margin-left: 4px;
    }

    .backdrop {
      position: fixed;
      inset: 0;
      background: var(--faso-overlay);
      z-index: var(--faso-z-drawer);
    }
    .drawer {
      position: fixed;
      top: 0;
      right: 0;
      height: 100dvh;
      width: min(420px, 100vw);
      background: var(--faso-surface);
      box-shadow: var(--faso-elevation-modal);
      z-index: calc(var(--faso-z-drawer) + 1);
      display: flex;
      flex-direction: column;
      animation: slide var(--faso-duration-normal) var(--faso-ease-decelerate);
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
      cursor: pointer;
      padding: 4px;
      border-radius: var(--faso-radius-md);
      color: var(--faso-text-muted);
    }
    .close:hover { background: var(--faso-surface-alt); }

    .drawer .body {
      flex: 1;
      overflow-y: auto;
      padding: var(--faso-space-5);
    }
    .drawer footer {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: var(--faso-space-2);
      padding: var(--faso-space-4) var(--faso-space-5);
      border-top: 1px solid var(--faso-border);
      background: var(--faso-surface-alt);
    }
  `],
})
export class FilterDrawerComponent {
  @Input() activeCount = 0;
  @Output() applyFilters = new EventEmitter<void>();
  @Output() clearFilters = new EventEmitter<void>();

  open = false;

  apply() { this.applyFilters.emit(); this.open = false; }
  clear() { this.clearFilters.emit(); }
}
