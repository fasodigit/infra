import { Component, OnInit, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterOutlet, RouterLink, RouterLinkActive, Router } from '@angular/router';
import { MatToolbarModule } from '@angular/material/toolbar';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatSidenavModule } from '@angular/material/sidenav';
import { MatListModule } from '@angular/material/list';
import { MatBadgeModule } from '@angular/material/badge';
import { MatMenuModule } from '@angular/material/menu';

import { AuthService } from './services/auth.service';
import { PanierService } from './services/panier.service';

@Component({
  selector: 'app-root',
  standalone: true,
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
  ],
  template: `
    <mat-sidenav-container class="app-container">
      <mat-sidenav #sidenav mode="over" class="app-sidenav">
        <mat-nav-list>
          <a mat-list-item routerLink="/" (click)="sidenav.close()">
            <mat-icon matListItemIcon>home</mat-icon>
            <span matListItemTitle>Accueil</span>
          </a>
          <a mat-list-item routerLink="/client/catalogue" (click)="sidenav.close()">
            <mat-icon matListItemIcon>storefront</mat-icon>
            <span matListItemTitle>Catalogue</span>
          </a>
          @if (auth.isAuthenticated()) {
            <a mat-list-item routerLink="/client/panier" (click)="sidenav.close()">
              <mat-icon matListItemIcon>shopping_cart</mat-icon>
              <span matListItemTitle>Panier</span>
            </a>
            <a mat-list-item routerLink="/client/commandes" (click)="sidenav.close()">
              <mat-icon matListItemIcon>receipt_long</mat-icon>
              <span matListItemTitle>Mes Commandes</span>
            </a>
          }
          @if (auth.isEleveur()) {
            <mat-divider></mat-divider>
            <h3 matSubheader>Espace Eleveur</h3>
            <a mat-list-item routerLink="/eleveur/dashboard" (click)="sidenav.close()">
              <mat-icon matListItemIcon>dashboard</mat-icon>
              <span matListItemTitle>Tableau de bord</span>
            </a>
            <a mat-list-item routerLink="/eleveur/poulets" (click)="sidenav.close()">
              <mat-icon matListItemIcon>egg_alt</mat-icon>
              <span matListItemTitle>Mes Poulets</span>
            </a>
          }
        </mat-nav-list>
      </mat-sidenav>

      <mat-sidenav-content>
        <!-- Toolbar -->
        <mat-toolbar color="primary" class="app-toolbar">
          <button mat-icon-button (click)="sidenav.toggle()">
            <mat-icon>menu</mat-icon>
          </button>

          <a routerLink="/" class="app-title">
            <span class="title-icon">🐔</span>
            <span>Poulets Platform</span>
          </a>

          <span class="toolbar-spacer"></span>

          <a mat-icon-button routerLink="/client/catalogue" matTooltip="Catalogue">
            <mat-icon>storefront</mat-icon>
          </a>

          @if (auth.isAuthenticated()) {
            <a mat-icon-button routerLink="/client/panier">
              <mat-icon [matBadge]="panier.itemCount()" matBadgeColor="accent"
                        [matBadgeHidden]="panier.itemCount() === 0">
                shopping_cart
              </mat-icon>
            </a>

            <button mat-icon-button [matMenuTriggerFor]="userMenu">
              <mat-icon>account_circle</mat-icon>
            </button>
            <mat-menu #userMenu="matMenu">
              <div class="user-menu-header" mat-menu-item disabled>
                <strong>{{ auth.currentUser()?.email }}</strong>
              </div>
              <mat-divider></mat-divider>
              @if (auth.isEleveur()) {
                <button mat-menu-item routerLink="/eleveur/dashboard">
                  <mat-icon>dashboard</mat-icon>
                  <span>Tableau de bord</span>
                </button>
              }
              <button mat-menu-item routerLink="/client/commandes">
                <mat-icon>receipt_long</mat-icon>
                <span>Mes Commandes</span>
              </button>
              <mat-divider></mat-divider>
              <button mat-menu-item (click)="onLogout()">
                <mat-icon>logout</mat-icon>
                <span>Deconnexion</span>
              </button>
            </mat-menu>
          } @else {
            <a mat-button routerLink="/login">Connexion</a>
            <a mat-raised-button color="accent" routerLink="/register">Inscription</a>
          }
        </mat-toolbar>

        <!-- Main content -->
        <main class="app-content">
          <router-outlet />
        </main>

        <!-- Footer -->
        <footer class="app-footer">
          <div class="container">
            <p>FASO DIGITALISATION - Poulets Platform v0.1.0</p>
          </div>
        </footer>
      </mat-sidenav-content>
    </mat-sidenav-container>
  `,
  styles: [`
    .app-container {
      height: 100vh;
    }

    .app-sidenav {
      width: 280px;
    }

    .app-toolbar {
      position: sticky;
      top: 0;
      z-index: 100;
      background-color: var(--faso-primary, #2e7d32);
    }

    .app-title {
      display: flex;
      align-items: center;
      gap: 8px;
      color: white;
      text-decoration: none;
      font-size: 1.2rem;
      font-weight: 500;
      margin-left: 8px;
    }

    .title-icon {
      font-size: 1.5rem;
    }

    .toolbar-spacer {
      flex: 1 1 auto;
    }

    .app-content {
      min-height: calc(100vh - 64px - 60px);
    }

    .app-footer {
      background-color: #333;
      color: #ccc;
      padding: 16px;
      text-align: center;
      font-size: 0.85rem;
    }

    .user-menu-header {
      padding: 8px 16px;
      opacity: 1 !important;
    }
  `],
})
export class AppComponent implements OnInit {
  readonly auth = inject(AuthService);
  readonly panier = inject(PanierService);
  private readonly router = inject(Router);

  ngOnInit(): void {
    this.auth.checkSession();
  }

  onLogout(): void {
    this.auth.logout().subscribe(() => {
      this.router.navigate(['/']);
    });
  }
}
