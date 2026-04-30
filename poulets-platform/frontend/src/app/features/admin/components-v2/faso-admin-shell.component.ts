// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, computed, input, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink, RouterOutlet } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';

import { FasoIconComponent, type IconName } from './faso-icon.component';
import { FasoRoleChipComponent } from './faso-role-chip.component';
import { FasoAvatarComponent } from './faso-avatar.component';
import type { AdminLang, AdminLevel, AdminUser, ThemeName } from '../models/admin.model';

interface NavItem {
  readonly id: string;
  readonly i18n: string;
  readonly icon: IconName;
  readonly route: string;
  readonly badge?: number;
}

@Component({
  selector: 'faso-admin-shell',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    RouterOutlet,
    TranslateModule,
    FasoIconComponent,
    FasoRoleChipComponent,
    FasoAvatarComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="fd-app" [attr.data-theme]="effectiveTheme()">
      <nav class="fd-sidebar">
        <div class="fd-brand">
          <div class="fd-brand-mark">FD</div>
          <div>
            <div class="fd-brand-name">FASO Digitalisation</div>
            <div class="fd-brand-sub">Admin · v2.4</div>
          </div>
        </div>

        <div class="fd-nav-section">{{ 'admin.nav.overview' | translate }}</div>
        @for (item of overviewItems; track item.id) {
          <a
            [routerLink]="item.route"
            class="fd-nav-item"
            [class.active]="active() === item.id"
          >
            <faso-icon [name]="item.icon" [size]="15"/>
            <span>{{ item.i18n | translate }}</span>
          </a>
        }

        <div class="fd-nav-section">{{ 'admin.nav.security' | translate }}</div>
        @for (item of securityItems(); track item.id) {
          <a
            [routerLink]="item.route"
            class="fd-nav-item"
            [class.active]="active() === item.id"
          >
            <faso-icon [name]="item.icon" [size]="15"/>
            <span>{{ item.i18n | translate }}</span>
            @if (item.badge) {
              <span class="fd-nav-badge">{{ item.badge }}</span>
            }
          </a>
        }

        <div class="fd-nav-section">{{ 'admin.nav.governance' | translate }}</div>
        @for (item of governanceItems; track item.id) {
          <a
            [routerLink]="item.route"
            class="fd-nav-item"
            [class.active]="active() === item.id"
          >
            <faso-icon [name]="item.icon" [size]="15"/>
            <span>{{ item.i18n | translate }}</span>
          </a>
        }

        <div style="flex: 1"></div>

        <div class="fd-nav-section">{{ 'admin.nav.me' | translate }}</div>
        @for (item of meItems; track item.id) {
          <a
            [routerLink]="item.route"
            class="fd-nav-item"
            [class.active]="active() === item.id"
          >
            <faso-icon [name]="item.icon" [size]="15"/>
            <span>{{ item.i18n | translate }}</span>
          </a>
        }

        <div class="fd-sidebar-foot">
          <faso-avatar [user]="footUser" [size]="28"/>
          <div class="fd-sidebar-foot-meta">
            <div class="fd-sidebar-foot-name">{{ footUser.firstName }} {{ footUser.lastName }}</div>
            <div class="fd-sidebar-foot-sub">{{ role() }} · session 7h 12min</div>
          </div>
        </div>
      </nav>

      <div class="fd-main">
        <div class="fd-topbar">
          <div class="fd-crumbs">
            @for (c of crumbs(); track $index; let i = $index, last = $last) {
              @if (i > 0) {
                <span class="fd-crumb-sep">/</span>
              }
              @if (last) {
                <strong>{{ c }}</strong>
              } @else {
                <span>{{ c }}</span>
              }
            }
          </div>

          <div class="fd-topbar-right">
            @if (breakGlass()) {
              <span class="fd-chip danger" style="font-size: 11px">
                <faso-icon name="flame" [size]="11"/> Break-Glass · 03:42:18
              </span>
            }
            <span class="fd-chip muted fd-mono" style="font-size: 11px">trace · 4f7c9e2a</span>
            <button type="button" class="fd-btn ghost icon" aria-label="Search" (click)="onSearch()">
              <faso-icon name="search" [size]="15"/>
            </button>
            <button
              type="button"
              class="fd-btn ghost icon"
              aria-label="Notifications"
              style="position: relative"
              (click)="onNotifications()"
            >
              <faso-icon name="bell" [size]="15"/>
              <span class="fd-topbar-bell-dot"></span>
            </button>
            <button
              type="button"
              class="fd-btn ghost icon"
              aria-label="Theme"
              (click)="toggleTheme()"
            >
              <faso-icon [name]="effectiveTheme() === 'dark' ? 'sun' : 'moon'" [size]="15"/>
            </button>
            <button
              type="button"
              class="fd-btn ghost"
              aria-label="Language"
              (click)="toggleLang()"
            >
              <faso-icon name="globe" [size]="14"/>
              <span style="font-size: 12px; font-weight: 600">{{ effectiveLang() | uppercase }}</span>
            </button>
            <faso-role-chip [role]="role()"/>
          </div>
        </div>

        <div class="fd-content">
          <router-outlet/>
        </div>
      </div>
    </div>
  `,
  styles: [`
    @use '../styles/admin-tokens';

    :host { display: block; width: 100%; height: 100%; }
    .fd-sidebar-foot {
      padding: 12px 10px;
      border-top: 1px solid var(--fd-border);
      display: flex;
      gap: 10px;
      align-items: center;
    }
    .fd-sidebar-foot-meta { min-width: 0; }
    .fd-sidebar-foot-name {
      font-size: 12.5px;
      font-weight: 600;
      line-height: 1.1;
    }
    .fd-sidebar-foot-sub {
      font-size: 11px;
      color: var(--fd-text-3);
    }
    .fd-topbar-bell-dot {
      position: absolute;
      top: 6px;
      right: 6px;
      width: 7px;
      height: 7px;
      border-radius: 50%;
      background: var(--fd-danger);
    }
  `],
})
export class FasoAdminShellComponent {
  readonly active = input<string>('');
  readonly crumbs = input<string[]>([]);
  readonly lang = input<AdminLang>('fr');
  readonly theme = input<ThemeName>('light');
  readonly role = input<AdminLevel>('SUPER-ADMIN');
  readonly breakGlass = input<boolean>(false);

  /** Overrides locaux quand l'utilisateur clique sur les toggles topbar. */
  private readonly themeOverride = signal<ThemeName | null>(null);
  private readonly langOverride = signal<AdminLang | null>(null);

  protected readonly effectiveTheme = computed<ThemeName>(
    () => this.themeOverride() ?? this.theme(),
  );
  protected readonly effectiveLang = computed<AdminLang>(
    () => this.langOverride() ?? this.lang(),
  );

  /** Items statiques du menu. Les clés i18n sont prises en charge par l'agent C. */
  protected readonly overviewItems: readonly NavItem[] = [
    { id: 'dashboard', i18n: 'admin.nav.dashboard', icon: 'grid',    route: '/admin/dashboard' },
    { id: 'users',     i18n: 'admin.nav.users',     icon: 'users',   route: '/admin/users' },
    { id: 'sessions',  i18n: 'admin.nav.sessions',  icon: 'monitor', route: '/admin/sessions' },
  ];

  protected readonly securityItems = computed<readonly NavItem[]>(() => [
    { id: 'devices',    i18n: 'admin.nav.devices',    icon: 'key',    route: '/admin/devices' },
    { id: 'mfa',        i18n: 'admin.nav.mfa',        icon: 'shield', route: '/admin/mfa' },
    { id: 'audit',      i18n: 'admin.nav.audit',      icon: 'log',    route: '/admin/audit', badge: 3 },
    { id: 'breakglass', i18n: 'admin.nav.breakglass', icon: 'flame',  route: '/admin/break-glass' },
  ]);

  protected readonly governanceItems: readonly NavItem[] = [
    { id: 'settings', i18n: 'admin.nav.settings', icon: 'settings', route: '/admin/settings' },
  ];

  protected readonly meItems: readonly NavItem[] = [
    { id: 'me', i18n: 'admin.nav.me', icon: 'user', route: '/admin/me/security' },
  ];

  /**
   * Stub piéton du user du pied de sidebar. Sera remplacé par un service
   * (SessionStore) côté pages quand le wiring sera fait.
   */
  protected readonly footUser: Pick<AdminUser, 'firstName' | 'lastName' | 'avatar'> = {
    firstName: 'Aminata',
    lastName: 'Ouédraogo',
    avatar: '#1b5e20',
  };

  protected toggleTheme(): void {
    const current = this.effectiveTheme();
    this.themeOverride.set(current === 'dark' ? 'light' : 'dark');
  }

  protected toggleLang(): void {
    const current = this.effectiveLang();
    this.langOverride.set(current === 'fr' ? 'en' : 'fr');
  }

  protected onSearch(): void {
    // Stub : implémenté ultérieurement (palette de commandes / recherche globale).
  }

  protected onNotifications(): void {
    // Stub : tiroir de notifications à brancher.
  }
}
