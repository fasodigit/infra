// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { CommonModule } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  input,
  signal,
} from '@angular/core';
import { RouterLink } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import {
  FasoAvatarComponent,
  FasoIconComponent,
  FasoRoleChipComponent,
} from '../components-v2';
import { AdminLang, AdminLevel } from '../models/admin.model';
import { MOCK_USERS } from '../services/admin-mocks';

@Component({
  selector: 'faso-users-list-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    TranslateModule,
    FasoIconComponent,
    FasoAvatarComponent,
    FasoRoleChipComponent,
  ],
  template: `
    <div class="fd-page-head">
      <div>
        <div class="fd-h1">
          {{ lang() === 'fr' ? 'Utilisateurs' : 'Users' }}
          <span
            style="font-weight: 400; color: var(--text-3); font-size: 16px;"
          >
            · 2 184
          </span>
        </div>
        <div class="fd-page-sub">
          {{
            lang() === 'fr'
              ? 'Comptes administrateurs et opérateurs des 9 applications sectorielles.'
              : 'Administrator and operator accounts across 9 sector applications.'
          }}
        </div>
      </div>
      <div class="fd-row">
        <button class="fd-btn">
          <faso-icon name="download" [size]="13" /> CSV
        </button>
        @if (canInvite()) {
          <button class="fd-btn primary">
            <faso-icon name="plus" [size]="13" />
            {{ lang() === 'fr' ? 'Inviter un admin' : 'Invite admin' }}
          </button>
        }
      </div>
    </div>

    <div class="fd-card" style="margin-bottom: 16px;">
      <div
        class="fd-card-b"
        style="display: flex; gap: 10px; align-items: center;"
      >
        <div style="flex: 1; position: relative;">
          <input
            class="fd-input search"
            [placeholder]="
              lang() === 'fr'
                ? 'Rechercher par nom, email, département…'
                : 'Search by name, email, department…'
            "
            [value]="searchQuery()"
            (input)="onSearchInput($event)"
          />
        </div>
        <span
          class="fd-chip role-super"
          style="cursor: pointer;"
          (click)="toggleRoleFilter('SUPER-ADMIN')"
        >
          SUPER-ADMIN
          <span style="margin-left: 4px; opacity: 0.6;">2</span>
        </span>
        <span
          class="fd-chip role-admin"
          style="cursor: pointer;"
          (click)="toggleRoleFilter('ADMIN')"
        >
          ADMIN
          <span style="margin-left: 4px; opacity: 0.6;">3</span>
        </span>
        <span
          class="fd-chip role-manager"
          style="cursor: pointer;"
          (click)="toggleRoleFilter('MANAGER')"
        >
          MANAGER
          <span style="margin-left: 4px; opacity: 0.6;">5</span>
        </span>
        <button class="fd-btn ghost sm">
          <faso-icon name="filter" [size]="13" />
          {{ lang() === 'fr' ? 'Filtres' : 'Filters' }}
        </button>
      </div>
    </div>

    <div class="fd-card">
      <table class="fd-table">
        <thead>
          <tr>
            <th>{{ lang() === 'fr' ? 'Utilisateur' : 'User' }}</th>
            <th>{{ lang() === 'fr' ? 'Département' : 'Department' }}</th>
            <th>{{ lang() === 'fr' ? 'Rôle' : 'Role' }}</th>
            <th>MFA</th>
            <th>{{ lang() === 'fr' ? 'Vérifié' : 'Verified' }}</th>
            <th>
              {{ lang() === 'fr' ? 'Dernière activité' : 'Last active' }}
            </th>
            <th style="text-align: right;">
              {{ lang() === 'fr' ? 'Actions' : 'Actions' }}
            </th>
          </tr>
        </thead>
        <tbody>
          @for (u of filteredUsers(); track u.id) {
            <tr>
              <td>
                <div class="fd-user-cell">
                  <faso-avatar [user]="u" [size]="32" />
                  <div>
                    <div class="fd-user-name">
                      {{ u.firstName }} {{ u.lastName }}
                    </div>
                    <div class="fd-user-email">{{ u.email }}</div>
                  </div>
                </div>
              </td>
              <td style="color: var(--text-2); font-size: 12.5px;">
                {{ u.department }}
              </td>
              <td><faso-role-chip [role]="u.role" /></td>
              <td>
                <div class="fd-row" style="gap: 6px;">
                  @if (u.mfa.passkey) {
                    <span
                      class="fd-chip ok"
                      style="font-size: 10.5px; padding: 1px 6px;"
                    >
                      <faso-icon name="key" [size]="10" /> PassKey
                    </span>
                  }
                  @if (u.mfa.totp) {
                    <span
                      class="fd-chip info"
                      style="font-size: 10.5px; padding: 1px 6px;"
                    >
                      <faso-icon name="qr" [size]="10" /> TOTP
                    </span>
                  }
                  @if (!u.mfa.passkey && !u.mfa.totp) {
                    <span
                      class="fd-chip danger"
                      style="font-size: 10.5px; padding: 1px 6px;"
                    >
                      —
                    </span>
                  }
                </div>
              </td>
              <td>
                @if (u.verified) {
                  <span class="fd-chip ok">
                    <faso-icon name="check" [size]="10" />
                    {{ lang() === 'fr' ? 'Oui' : 'Yes' }}
                  </span>
                } @else {
                  <span class="fd-chip warn">
                    <faso-icon name="clock" [size]="10" />
                    {{ lang() === 'fr' ? 'En attente' : 'Pending' }}
                  </span>
                }
              </td>
              <td
                [style.color]="
                  u.status === 'suspended' ? 'var(--danger)' : 'var(--text-2)'
                "
                style="font-size: 12.5px;"
              >
                {{
                  u.status === 'suspended'
                    ? lang() === 'fr'
                      ? 'Suspendu'
                      : 'Suspended'
                    : u.lastActive
                }}
              </td>
              <td>
                <div class="actions">
                  <button
                    class="fd-btn ghost sm"
                    [routerLink]="['..', 'users', u.id]"
                  >
                    <faso-icon name="eye" [size]="12" />
                  </button>
                  <button class="fd-btn sm">
                    {{ lang() === 'fr' ? 'Gérer rôles' : 'Manage roles' }}
                  </button>
                  <button class="fd-btn ghost sm">
                    <faso-icon name="moreH" [size]="14" />
                  </button>
                </div>
              </td>
            </tr>
          }
        </tbody>
      </table>
      <div
        style="padding: 12px 18px; border-top: 1px solid var(--border); display: flex; justify-content: space-between; font-size: 12px; color: var(--text-3);"
      >
        <span>
          {{
            lang() === 'fr'
              ? 'Affichage 1–10 sur 2 184'
              : 'Showing 1–10 of 2,184'
          }}
          · cdk-virtual-scroll
        </span>
        <div class="fd-row">
          <button class="fd-btn ghost sm" disabled>
            <faso-icon
              name="chevR"
              [size]="12"
              style="transform: rotate(180deg);"
            />
          </button>
          <span class="fd-mono">1 / 219</span>
          <button class="fd-btn ghost sm">
            <faso-icon name="chevR" [size]="12" />
          </button>
        </div>
      </div>
    </div>
  `,
  styles: [
    `
      :host {
        display: contents;
      }
    `,
  ],
})
export class UsersListPage {
  readonly lang = input<AdminLang>('fr');
  readonly role = input<AdminLevel>('SUPER-ADMIN');

  protected readonly users = signal(MOCK_USERS);
  protected readonly searchQuery = signal('');
  protected readonly activeRoleFilter = signal<AdminLevel | null>(null);

  protected readonly canInvite = computed(
    () => this.role() === 'SUPER-ADMIN' || this.role() === 'ADMIN',
  );

  protected readonly filteredUsers = computed(() => {
    const q = this.searchQuery().toLowerCase().trim();
    const role = this.activeRoleFilter();
    return this.users().filter((u) => {
      if (role && u.role !== role) return false;
      if (!q) return true;
      return (
        u.firstName.toLowerCase().includes(q) ||
        u.lastName.toLowerCase().includes(q) ||
        u.email.toLowerCase().includes(q) ||
        u.department.toLowerCase().includes(q)
      );
    });
  });

  protected onSearchInput(event: Event): void {
    const value = (event.target as HTMLInputElement).value;
    this.searchQuery.set(value);
  }

  protected toggleRoleFilter(role: AdminLevel): void {
    this.activeRoleFilter.update((current) => (current === role ? null : role));
  }
}
