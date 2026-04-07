import { Component, inject, signal, ChangeDetectionStrategy, ViewChild } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterOutlet, RouterLink, RouterLinkActive, Router } from '@angular/router';
import { BreakpointObserver, Breakpoints } from '@angular/cdk/layout';
import { MatToolbarModule } from '@angular/material/toolbar';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatSidenavModule, MatSidenav } from '@angular/material/sidenav';
import { MatListModule } from '@angular/material/list';
import { MatBadgeModule } from '@angular/material/badge';
import { MatMenuModule } from '@angular/material/menu';
import { MatDividerModule } from '@angular/material/divider';
import { MatTooltipModule } from '@angular/material/tooltip';
import { TranslateModule } from '@ngx-translate/core';

import { AuthService } from '@core/services/auth.service';
import { LanguageSwitcherComponent } from '@shared/components/language-switcher/language-switcher.component';

@Component({
  selector: 'app-layout',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterOutlet,
    RouterLink,
    RouterLinkActive,
    MatToolbarModule,
    MatButtonModule,
    MatIconModule,
    MatSidenavModule,
    MatListModule,
    MatBadgeModule,
    MatMenuModule,
    MatDividerModule,
    MatTooltipModule,
    TranslateModule,
    LanguageSwitcherComponent,
  ],
  template: `
    <mat-sidenav-container class="layout-container">
      <!-- Sidebar -->
      <mat-sidenav
        #sidenav
        [mode]="isMobile() ? 'over' : 'side'"
        [opened]="!isMobile()"
        class="layout-sidenav"
      >
        <!-- Sidebar header -->
        <div class="sidenav-header">
          <span class="logo-text">Poulets BF</span>
        </div>

        <mat-divider></mat-divider>

        @if (auth.isLoggedIn()) {
          <div class="space-label">
            {{ auth.spaceLabel() | translate }}
          </div>
        }

        <mat-nav-list>
          @for (item of auth.menuItems(); track item.route) {
            <a
              mat-list-item
              [routerLink]="item.route"
              routerLinkActive="active-link"
              (click)="onNavClick()"
            >
              <mat-icon matListItemIcon>{{ item.icon }}</mat-icon>
              <span matListItemTitle>{{ item.labelKey | translate }}</span>
            </a>
          }
        </mat-nav-list>
      </mat-sidenav>

      <!-- Main content area -->
      <mat-sidenav-content>
        <!-- Top toolbar -->
        <mat-toolbar class="layout-toolbar">
          <button mat-icon-button (click)="sidenav.toggle()" [attr.aria-label]="'menu'">
            <mat-icon>menu</mat-icon>
          </button>

          <a routerLink="/dashboard" class="toolbar-title">
            <span>Poulets BF</span>
          </a>

          <span class="toolbar-spacer"></span>

          <!-- Language switcher -->
          <app-language-switcher></app-language-switcher>

          <!-- Notification bell -->
          <button mat-icon-button [matTooltip]="'common.notifications' | translate">
            <mat-icon [matBadge]="notificationCount()" matBadgeColor="warn"
                      [matBadgeHidden]="notificationCount() === 0" matBadgeSize="small">
              notifications
            </mat-icon>
          </button>

          <!-- User menu -->
          @if (auth.isLoggedIn()) {
            <button mat-icon-button [matMenuTriggerFor]="userMenu">
              <mat-icon>account_circle</mat-icon>
            </button>
            <mat-menu #userMenu="matMenu">
              <div class="user-menu-header" mat-menu-item disabled>
                <strong>{{ auth.currentUser()?.nom }}</strong>
                <br>
                <small>{{ auth.currentUser()?.email }}</small>
              </div>
              <mat-divider></mat-divider>
              <button mat-menu-item routerLink="/profile">
                <mat-icon>person</mat-icon>
                <span>{{ 'menu.profile' | translate }}</span>
              </button>
              <button mat-menu-item routerLink="/messaging">
                <mat-icon>chat</mat-icon>
                <span>{{ 'menu.messaging' | translate }}</span>
              </button>
              <mat-divider></mat-divider>
              <button mat-menu-item (click)="onLogout()">
                <mat-icon>logout</mat-icon>
                <span>{{ 'auth.logout' | translate }}</span>
              </button>
            </mat-menu>
          }
        </mat-toolbar>

        <!-- Router outlet for child routes -->
        <main class="layout-content">
          <router-outlet />
        </main>

        <!-- Footer -->
        <footer class="layout-footer">
          <span>{{ 'app.footer' | translate }}</span>
        </footer>
      </mat-sidenav-content>
    </mat-sidenav-container>
  `,
  styles: [`
    .layout-container {
      height: 100vh;
    }

    .layout-sidenav {
      width: 260px;
      background-color: #1b5e20;
    }

    .sidenav-header {
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 20px 16px;
      background-color: #1b5e20;
    }

    .logo-text {
      font-size: 1.5rem;
      font-weight: 700;
      color: #ffffff;
      letter-spacing: 0.5px;
    }

    .space-label {
      padding: 12px 16px 4px;
      font-size: 0.75rem;
      font-weight: 500;
      text-transform: uppercase;
      letter-spacing: 1px;
      color: rgba(255, 255, 255, 0.6);
    }

    .layout-sidenav mat-nav-list a {
      color: rgba(255, 255, 255, 0.85);
    }

    .layout-sidenav mat-nav-list a:hover {
      background-color: rgba(255, 255, 255, 0.08);
    }

    .layout-sidenav mat-nav-list a.active-link {
      background-color: rgba(255, 255, 255, 0.15);
      color: #ffffff;
      border-left: 3px solid #ff8f00;
    }

    .layout-sidenav mat-nav-list mat-icon {
      color: rgba(255, 255, 255, 0.7);
    }

    .layout-sidenav mat-nav-list a.active-link mat-icon {
      color: #ff8f00;
    }

    .layout-sidenav mat-divider {
      border-top-color: rgba(255, 255, 255, 0.12);
    }

    .layout-toolbar {
      position: sticky;
      top: 0;
      z-index: 100;
      background-color: var(--faso-primary, #2e7d32);
      color: white;
    }

    .toolbar-title {
      display: flex;
      align-items: center;
      gap: 8px;
      color: white;
      text-decoration: none;
      font-size: 1.15rem;
      font-weight: 500;
      margin-left: 8px;
    }

    .toolbar-spacer {
      flex: 1 1 auto;
    }

    .layout-content {
      min-height: calc(100vh - 64px - 48px);
      background-color: var(--faso-bg, #fafafa);
    }

    .layout-footer {
      background-color: #333;
      color: #ccc;
      padding: 14px 16px;
      text-align: center;
      font-size: 0.85rem;
    }

    .user-menu-header {
      padding: 8px 16px;
      line-height: 1.4;
      opacity: 1 !important;
    }

    .user-menu-header small {
      color: #999;
    }
  `],
})
export class LayoutComponent {
  readonly auth = inject(AuthService);
  private readonly router = inject(Router);
  private readonly breakpointObserver = inject(BreakpointObserver);

  @ViewChild('sidenav') sidenav!: MatSidenav;

  readonly isMobile = signal(false);
  readonly notificationCount = signal(0);

  constructor() {
    this.breakpointObserver
      .observe([Breakpoints.Handset, Breakpoints.TabletPortrait])
      .subscribe((result) => {
        this.isMobile.set(result.matches);
      });
  }

  onNavClick(): void {
    if (this.isMobile()) {
      this.sidenav?.close();
    }
  }

  onLogout(): void {
    this.auth.logout().subscribe(() => {
      this.router.navigate(['/auth/login']);
    });
  }
}
