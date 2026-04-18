// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, computed, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatMenuModule } from '@angular/material/menu';

import { LoadingComponent } from '@shared/components/loading/loading.component';
import { PlatformUser } from '@shared/models/admin.models';
import { UserRole } from '@core/config/project-config.token';
import { UsersService } from '../services/users.service';

@Component({
  selector: 'app-users-list',
  standalone: true,
  imports: [
    CommonModule, DatePipe, FormsModule, RouterLink,
    MatIconModule, MatButtonModule, MatMenuModule,
    LoadingComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <div>
          <h1>Gestion des utilisateurs</h1>
          <p>{{ total() }} comptes · 4 rôles</p>
        </div>
        <a mat-raised-button color="primary" routerLink="create">
          <mat-icon>person_add</mat-icon>
          Créer un utilisateur
        </a>
      </header>

      <div class="toolbar">
        <label class="search">
          <mat-icon>search</mat-icon>
          <input type="search" [ngModel]="search()" (ngModelChange)="onSearchChange($event)" placeholder="Rechercher par nom, email, téléphone…">
        </label>
        <div class="role-tabs" role="tablist">
          <button
            type="button"
            role="tab"
            [class.active]="roleFilter() === ''"
            (click)="setRole('')"
          >Tous ({{ countBy('') }})</button>
          <button type="button" role="tab" [class.active]="roleFilter() === 'ELEVEUR'"    (click)="setRole('ELEVEUR')">Éleveurs ({{ countBy('ELEVEUR') }})</button>
          <button type="button" role="tab" [class.active]="roleFilter() === 'CLIENT'"     (click)="setRole('CLIENT')">Clients ({{ countBy('CLIENT') }})</button>
          <button type="button" role="tab" [class.active]="roleFilter() === 'PRODUCTEUR'" (click)="setRole('PRODUCTEUR')">Producteurs ({{ countBy('PRODUCTEUR') }})</button>
          <button type="button" role="tab" [class.active]="roleFilter() === 'ADMIN'"      (click)="setRole('ADMIN')">Admins ({{ countBy('ADMIN') }})</button>
        </div>
      </div>

      @if (loading()) {
        <app-loading message="Chargement des utilisateurs…" />
      } @else if (filtered().length === 0) {
        <div class="empty">
          <mat-icon>person_search</mat-icon>
          <p>Aucun utilisateur ne correspond aux filtres.</p>
        </div>
      } @else {
        <div class="cards">
          @for (u of filtered(); track u.id) {
            <article class="user-card" [class.inactive]="!u.isActive">
              <div class="identity">
                <span class="initials">{{ initials(u) }}</span>
                <div>
                  <a [routerLink]="[u.id]"><strong>{{ u.displayName }}</strong></a>
                  <span>{{ u.email }}</span>
                </div>
              </div>
              <div class="role-col">
                <span class="role-badge" [class]="'role--' + u.role.toLowerCase()">{{ roleLabel(u.role) }}</span>
                @if (u.region) { <small>{{ u.region }}</small> }
              </div>
              <div class="mfa-col" [title]="mfaTooltip(u)">
                @for (m of mfaFlags(u); track m.key) {
                  <span class="mfa-pill" [class.on]="m.on">
                    <mat-icon>{{ m.icon }}</mat-icon>
                  </span>
                }
              </div>
              <div class="status-col">
                <span class="status" [class.active]="u.isActive">
                  <span class="dot"></span>
                  {{ u.isActive ? 'Actif' : 'Inactif' }}
                </span>
                @if (u.lastLoginAt) {
                  <small>Connecté {{ u.lastLoginAt | date:'short' }}</small>
                }
              </div>
              <div class="actions-col">
                <a mat-icon-button [routerLink]="[u.id]" aria-label="Voir détails">
                  <mat-icon>open_in_new</mat-icon>
                </a>
                <button mat-icon-button [matMenuTriggerFor]="menu" aria-label="Actions">
                  <mat-icon>more_vert</mat-icon>
                </button>
                <mat-menu #menu="matMenu">
                  <a mat-menu-item [routerLink]="[u.id, 'edit']">
                    <mat-icon>edit</mat-icon><span>Modifier</span>
                  </a>
                  @if (u.isActive) {
                    <button mat-menu-item (click)="deactivate(u)">
                      <mat-icon>block</mat-icon><span>Désactiver</span>
                    </button>
                  } @else {
                    <button mat-menu-item (click)="reactivate(u)">
                      <mat-icon>check_circle</mat-icon><span>Réactiver</span>
                    </button>
                  }
                  <button mat-menu-item (click)="forceLogout(u)">
                    <mat-icon>logout</mat-icon><span>Forcer déconnexion</span>
                  </button>
                </mat-menu>
              </div>
            </article>
          }
        </div>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    header {
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-5);
      flex-wrap: wrap;
    }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .toolbar {
      display: flex;
      gap: var(--faso-space-3);
      align-items: center;
      margin-bottom: var(--faso-space-4);
      flex-wrap: wrap;
    }
    .search {
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 6px 12px;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-pill);
      flex: 1;
      min-width: 260px;
    }
    .search mat-icon { color: var(--faso-text-muted); font-size: 18px; width: 18px; height: 18px; }
    .search input {
      flex: 1;
      border: none;
      outline: none;
      background: transparent;
      font-family: inherit;
      font-size: var(--faso-text-sm);
      color: var(--faso-text);
    }
    .role-tabs {
      display: inline-flex;
      gap: 4px;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-pill);
      padding: 4px;
      flex-wrap: wrap;
    }
    .role-tabs button {
      padding: 6px 14px;
      border: none;
      background: transparent;
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-medium);
      cursor: pointer;
      color: var(--faso-text-muted);
    }
    .role-tabs button.active {
      background: var(--faso-primary-600);
      color: var(--faso-text-inverse);
    }

    .empty {
      padding: var(--faso-space-10);
      text-align: center;
      color: var(--faso-text-muted);
    }
    .empty mat-icon { font-size: 48px; width: 48px; height: 48px; color: var(--faso-text-subtle); }

    .cards { display: flex; flex-direction: column; gap: var(--faso-space-2); }
    .user-card {
      display: grid;
      grid-template-columns: 2fr 1fr 1fr 1fr auto;
      gap: var(--faso-space-3);
      align-items: center;
      padding: var(--faso-space-3) var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
    }
    .user-card.inactive { opacity: 0.6; }

    .identity {
      display: flex;
      align-items: center;
      gap: var(--faso-space-3);
      min-width: 0;
    }
    .initials {
      width: 36px; height: 36px;
      border-radius: 50%;
      background: var(--faso-primary-100);
      color: var(--faso-primary-700);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      font-weight: var(--faso-weight-semibold);
      flex-shrink: 0;
    }
    .identity a { color: var(--faso-text); text-decoration: none; }
    .identity a:hover { color: var(--faso-primary-700); }
    .identity strong { display: block; }
    .identity span {
      display: block;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .role-col { display: flex; flex-direction: column; gap: 4px; }
    .role-badge {
      display: inline-flex;
      padding: 2px 10px;
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      align-self: flex-start;
    }
    .role--eleveur    { background: var(--faso-primary-50);  color: var(--faso-primary-700); }
    .role--client     { background: var(--faso-info-bg);     color: var(--faso-info); }
    .role--producteur { background: var(--faso-accent-100);  color: var(--faso-accent-800); }
    .role--admin      { background: var(--faso-warning-bg);  color: var(--faso-warning); }
    .role-col small { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); }

    .mfa-col { display: inline-flex; gap: 4px; }
    .mfa-pill {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 24px; height: 24px;
      border-radius: 50%;
      background: var(--faso-surface-alt);
      color: var(--faso-text-subtle);
      border: 1px solid var(--faso-border);
    }
    .mfa-pill mat-icon { font-size: 14px; width: 14px; height: 14px; }
    .mfa-pill.on {
      background: var(--faso-success-bg);
      color: var(--faso-success);
      border-color: var(--faso-success);
    }

    .status-col { display: flex; flex-direction: column; gap: 2px; }
    .status {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      font-size: var(--faso-text-sm);
      color: var(--faso-text-muted);
    }
    .status .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--faso-text-subtle); }
    .status.active { color: var(--faso-success); }
    .status.active .dot { background: var(--faso-success); box-shadow: 0 0 0 3px var(--faso-success-bg); }
    .status-col small { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); }

    .actions-col { display: inline-flex; align-items: center; gap: 2px; }

    @media (max-width: 899px) {
      .user-card { grid-template-columns: 1fr auto; row-gap: var(--faso-space-2); }
      .role-col, .mfa-col, .status-col { grid-column: 1 / -1; }
    }
  `],
})
export class UsersListComponent implements OnInit {
  private readonly svc = inject(UsersService);

  readonly users = signal<PlatformUser[]>([]);
  readonly loading = signal(true);
  readonly total = signal(0);
  readonly search = signal('');
  readonly roleFilter = signal<UserRole | ''>('');

  readonly filtered = computed(() => {
    const q = this.search().trim().toLowerCase();
    const role = this.roleFilter();
    return this.users().filter((u) => {
      if (role && u.role !== role) return false;
      if (q) {
        const blob = `${u.displayName} ${u.email} ${u.phone ?? ''} ${u.region ?? ''}`.toLowerCase();
        if (!blob.includes(q)) return false;
      }
      return true;
    });
  });

  ngOnInit(): void { this.load(); }

  setRole(r: UserRole | ''): void { this.roleFilter.set(r); }
  onSearchChange(v: string): void { this.search.set(v); }

  countBy(r: UserRole | ''): number {
    return r === '' ? this.users().length : this.users().filter((u) => u.role === r).length;
  }

  initials(u: PlatformUser): string {
    return [u.firstName, u.lastName].filter(Boolean).map((n) => n!.charAt(0).toUpperCase()).join('');
  }

  roleLabel(r: UserRole): string {
    switch (r) {
      case 'ELEVEUR':    return 'Éleveur';
      case 'CLIENT':     return 'Client';
      case 'PRODUCTEUR': return 'Producteur';
      case 'ADMIN':      return 'Admin';
    }
  }

  mfaFlags(u: PlatformUser) {
    return [
      { key: 'email',       icon: 'mail',          on: u.mfaStatus.email },
      { key: 'passkey',     icon: 'fingerprint',   on: u.mfaStatus.passkey },
      { key: 'totp',        icon: 'qr_code_2',     on: u.mfaStatus.totp },
      { key: 'backupCodes', icon: 'vpn_key',       on: u.mfaStatus.backupCodes },
    ];
  }

  mfaTooltip(u: PlatformUser): string {
    const enabled = Object.entries(u.mfaStatus).filter(([, v]) => v).map(([k]) => k);
    return enabled.length ? 'MFA actif : ' + enabled.join(', ') : 'Aucune méthode MFA configurée';
  }

  deactivate(u: PlatformUser) { this.svc.deactivate(u.id).subscribe(() => this.load()); }
  reactivate(u: PlatformUser) { this.svc.reactivate(u.id).subscribe(() => this.load()); }
  forceLogout(u: PlatformUser) { this.svc.forceLogout(u.id).subscribe(); }

  private load(): void {
    this.loading.set(true);
    this.svc.list(0, 100).subscribe({
      next: (p) => {
        this.users.set(p.content);
        this.total.set(p.totalElements);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }
}
