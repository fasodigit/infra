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
import { TranslateModule } from '@ngx-translate/core';
import {
  FasoAvatarComponent,
  FasoIconComponent,
  FasoRoleChipComponent,
} from '../components-v2';
import { AdminLang, AdminSession } from '../models/admin.model';
import { MOCK_SESSIONS, MOCK_USERS } from '../services/admin-mocks';

@Component({
  selector: 'faso-sessions-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    TranslateModule,
    FasoIconComponent,
    FasoAvatarComponent,
    FasoRoleChipComponent,
  ],
  template: `
    <div class="fd-page-head">
      <div>
        <div class="fd-h1">
          {{ lang() === 'fr' ? 'Sessions actives' : 'Active sessions' }}
          <span
            style="font-weight: 400; color: var(--text-3); font-size: 16px;"
          >
            · 58
          </span>
        </div>
        <div class="fd-page-sub">
          {{
            lang() === 'fr'
              ? 'Vue temps-réel via KAYA · auth:sessions:* — révocation propagée à ARMAGEDDON via Redpanda.'
              : 'Real-time view via KAYA · revocation propagated to ARMAGEDDON via Redpanda.'
          }}
        </div>
      </div>
      <div class="fd-row">
        <button class="fd-btn">
          <faso-icon name="refresh" [size]="13" />
          {{ lang() === 'fr' ? 'Actualiser' : 'Refresh' }}
        </button>
        <button class="fd-btn danger">
          <faso-icon name="logout" [size]="13" />
          {{ lang() === 'fr' ? 'Tout révoquer' : 'Revoke all' }}
        </button>
      </div>
    </div>

    <div class="fd-banner info">
      <faso-icon name="info" [size]="16" />
      <div class="fd-banner-body">
        @if (lang() === 'fr') {
          <span>
            <strong>3 sessions</strong> ont dépassé le seuil
            <span class="fd-mono">session.max_concurrent_per_user = 3</span>.
            Les sessions les plus anciennes seront automatiquement révoquées au
            prochain événement.
          </span>
        } @else {
          <span>
            <strong>3 sessions</strong> exceed
            <span class="fd-mono">session.max_concurrent_per_user = 3</span>.
            Oldest will auto-revoke on next event.
          </span>
        }
      </div>
    </div>

    <div class="fd-card">
      <table class="fd-table">
        <thead>
          <tr>
            <th>{{ lang() === 'fr' ? 'Utilisateur' : 'User' }}</th>
            <th>Session ID</th>
            <th>{{ lang() === 'fr' ? 'Créée' : 'Created' }}</th>
            <th>
              {{ lang() === 'fr' ? 'Dernière activité' : 'Last active' }}
            </th>
            <th>{{ lang() === 'fr' ? 'Origine' : 'Origin' }}</th>
            <th>{{ lang() === 'fr' ? 'Appareil' : 'Device' }}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          @for (s of sessions(); track s.id) {
            <tr>
              <td>
                @if (userOf(s); as u) {
                  <div class="fd-user-cell">
                    <faso-avatar [user]="u" [size]="28" />
                    <div>
                      <div style="font-size: 12.5px; font-weight: 500;">
                        {{ u.firstName }} {{ u.lastName }}
                      </div>
                      <div style="font-size: 11px; color: var(--text-3);">
                        <faso-role-chip [role]="u.role" />
                      </div>
                    </div>
                  </div>
                }
              </td>
              <td>
                <span class="fd-mono-pill">{{ s.token }}</span>
                @if (s.current) {
                  <span
                    class="fd-chip ok"
                    style="margin-left: 6px; font-size: 10px;"
                  >
                    {{ lang() === 'fr' ? 'actuelle' : 'current' }}
                  </span>
                }
              </td>
              <td
                class="fd-mono"
                style="font-size: 12px; color: var(--text-2);"
              >
                {{ s.created }}
              </td>
              <td style="font-size: 12.5px;">{{ s.lastActive }}</td>
              <td>
                <div class="fd-mono" style="font-size: 12px;">{{ s.ip }}</div>
                <div style="font-size: 11px; color: var(--text-3);">
                  <faso-icon name="globe" [size]="10" /> {{ s.city }}
                </div>
              </td>
              <td style="font-size: 12px; color: var(--text-2);">
                {{ s.device }}
              </td>
              <td>
                <button
                  class="fd-btn ghost sm"
                  style="color: var(--danger);"
                >
                  <faso-icon name="logout" [size]="12" />
                  {{ lang() === 'fr' ? 'Logout' : 'Logout' }}
                </button>
              </td>
            </tr>
          }
        </tbody>
      </table>
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
export class SessionsPage {
  readonly lang = input<AdminLang>('fr');

  protected readonly sessions = signal(MOCK_SESSIONS);
  protected readonly users = signal(MOCK_USERS);

  protected userOf(session: AdminSession) {
    return this.users().find((u) => u.id === session.user);
  }
}
