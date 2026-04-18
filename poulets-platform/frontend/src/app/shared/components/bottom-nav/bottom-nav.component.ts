// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink, RouterLinkActive } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';

export interface BottomNavItem {
  route: string | any[];
  icon: string;
  label: string;
  exact?: boolean;
}

@Component({
  selector: 'app-bottom-nav',
  standalone: true,
  imports: [CommonModule, RouterLink, RouterLinkActive, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <nav class="nav" role="navigation" aria-label="Navigation principale">
      @for (item of items; track item.route) {
        <a
          [routerLink]="item.route"
          [routerLinkActiveOptions]="{ exact: !!item.exact }"
          routerLinkActive="active"
          class="item"
        >
          <mat-icon>{{ item.icon }}</mat-icon>
          <span>{{ item.label }}</span>
        </a>
      }
    </nav>
  `,
  styles: [`
    :host {
      display: none;
      position: fixed;
      bottom: 0;
      left: 0;
      right: 0;
      z-index: var(--faso-z-sticky);
    }

    .nav {
      display: grid;
      grid-auto-flow: column;
      grid-auto-columns: 1fr;
      background: var(--faso-surface);
      border-top: 1px solid var(--faso-border);
      box-shadow: 0 -2px 8px rgba(15, 23, 42, 0.05);
      padding-bottom: env(safe-area-inset-bottom, 0);
    }

    .item {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 2px;
      padding: 8px 4px;
      color: var(--faso-text-muted);
      text-decoration: none;
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-medium);
      transition: color var(--faso-duration-fast) var(--faso-ease-standard);
      min-height: var(--faso-bottom-nav-height);
    }

    .item mat-icon {
      font-size: 24px;
      width: 24px;
      height: 24px;
    }

    .item.active {
      color: var(--faso-primary-600);
    }

    .item:hover { color: var(--faso-primary-700); text-decoration: none; }

    :host {
      @media (max-width: 767.98px) {
        display: block;
      }
    }
  `],
})
export class BottomNavComponent {
  @Input() items: BottomNavItem[] = [
    { route: '/dashboard', icon: 'home', label: 'Accueil' },
    { route: '/marketplace/annonces', icon: 'search', label: 'Recherche' },
    { route: '/orders', icon: 'receipt_long', label: 'Commandes' },
    { route: '/messaging', icon: 'chat', label: 'Messages' },
    { route: '/profile', icon: 'person', label: 'Profil' },
  ];
}
