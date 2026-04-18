// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { BreadcrumbComponent, BreadcrumbItem } from '../breadcrumb/breadcrumb.component';

interface AdminNavEntry {
  label: string;
  icon: string;
  route: string;
}

const DEFAULT_ADMIN_NAV: AdminNavEntry[] = [
  { label: 'Monitoring',       icon: 'monitoring',      route: '/admin/monitoring' },
  { label: 'Utilisateurs',     icon: 'group',           route: '/admin/users' },
  { label: "Logs d'audit",     icon: 'fact_check',      route: '/admin/audit' },
  { label: 'Configuration',    icon: 'tune',            route: '/admin/platform-config' },
];

@Component({
  selector: 'app-admin-shell',
  standalone: true,
  imports: [CommonModule, RouterLink, RouterLinkActive, RouterOutlet, MatIconModule, BreadcrumbComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="shell">
      <aside class="side">
        <header>
          <mat-icon>admin_panel_settings</mat-icon>
          <span>Administration</span>
        </header>
        <nav>
          @for (item of navItems; track item.route) {
            <a
              [routerLink]="item.route"
              routerLinkActive="active"
              [routerLinkActiveOptions]="{ exact: false }"
            >
              <mat-icon>{{ item.icon }}</mat-icon>
              {{ item.label }}
            </a>
          }
        </nav>
      </aside>

      <section class="content">
        @if (breadcrumb.length > 0) {
          <app-breadcrumb [items]="breadcrumb" />
        }
        <router-outlet />
      </section>
    </div>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .shell {
      display: grid;
      grid-template-columns: 240px 1fr;
      min-height: 100vh;
    }
    .side {
      background: var(--faso-surface);
      border-right: 1px solid var(--faso-border);
      padding: var(--faso-space-5) 0;
      position: sticky;
      top: 0;
      height: 100vh;
      overflow-y: auto;
    }
    .side header {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 0 var(--faso-space-5) var(--faso-space-4);
      border-bottom: 1px solid var(--faso-border);
      margin-bottom: var(--faso-space-3);
      color: var(--faso-primary-700);
      font-weight: var(--faso-weight-semibold);
    }
    .side nav {
      display: flex;
      flex-direction: column;
      gap: 2px;
      padding: 0 var(--faso-space-3);
    }
    .side nav a {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 10px 12px;
      border-radius: var(--faso-radius-md);
      color: var(--faso-text-muted);
      text-decoration: none;
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-medium);
      transition: background var(--faso-duration-fast) var(--faso-ease-standard),
                  color var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .side nav a:hover {
      background: var(--faso-surface-alt);
      color: var(--faso-text);
      text-decoration: none;
    }
    .side nav a.active {
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      font-weight: var(--faso-weight-semibold);
    }
    .side nav a mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    .content {
      padding: var(--faso-space-6) var(--faso-space-5) var(--faso-space-12);
      max-width: 1400px;
      width: 100%;
    }
    app-breadcrumb { margin-bottom: var(--faso-space-4); display: block; }

    @media (max-width: 899px) {
      .shell { grid-template-columns: 1fr; }
      .side {
        position: static;
        height: auto;
        border-right: none;
        border-bottom: 1px solid var(--faso-border);
      }
      .side nav { flex-direction: row; overflow-x: auto; }
      .side nav a { white-space: nowrap; }
    }
  `],
})
export class AdminShellComponent {
  @Input() navItems: AdminNavEntry[] = DEFAULT_ADMIN_NAV;
  @Input() breadcrumb: BreadcrumbItem[] = [];
}
