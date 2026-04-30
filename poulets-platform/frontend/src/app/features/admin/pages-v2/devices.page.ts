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
} from '../components-v2';
import { AdminLang, TrustedDevice } from '../models/admin.model';
import { MOCK_DEVICES, MOCK_USERS } from '../services/admin-mocks';

@Component({
  selector: 'faso-devices-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    TranslateModule,
    FasoIconComponent,
    FasoAvatarComponent,
  ],
  template: `
    <div class="fd-page-head">
      <div>
        <div class="fd-h1">
          {{ lang() === 'fr' ? 'Appareils trustés' : 'Trusted devices' }}
          <span
            style="font-weight: 400; color: var(--text-3); font-size: 16px;"
          >
            · 247
          </span>
        </div>
        <div class="fd-page-sub">
          {{
            lang() === 'fr'
              ? 'Empreintes UA + IP/24 + Accept-Language · TTL 30 jours dans KAYA.'
              : 'UA + IP/24 + Accept-Language fingerprints · 30-day TTL in KAYA.'
          }}
        </div>
      </div>
      <div class="fd-row">
        <button class="fd-btn">
          <faso-icon name="filter" [size]="13" />
          {{ lang() === 'fr' ? 'Filtrer' : 'Filter' }}
        </button>
        <button class="fd-btn">
          <faso-icon name="download" [size]="13" /> CSV
        </button>
      </div>
    </div>

    <div class="fd-card">
      <table class="fd-table">
        <thead>
          <tr>
            <th>{{ lang() === 'fr' ? 'Utilisateur' : 'User' }}</th>
            <th>{{ lang() === 'fr' ? 'Empreinte' : 'Fingerprint' }}</th>
            <th>{{ lang() === 'fr' ? 'Type' : 'Type' }}</th>
            <th>UA</th>
            <th>IP / {{ lang() === 'fr' ? 'Ville' : 'City' }}</th>
            <th>{{ lang() === 'fr' ? 'Dernier accès' : 'Last accessed' }}</th>
            <th>
              {{ lang() === 'fr' ? "Trust jusqu'à" : 'Trusted until' }}
            </th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          @for (d of devices(); track d.id) {
            <tr>
              <td>
                @if (userOf(d); as u) {
                  <div class="fd-user-cell">
                    <faso-avatar [user]="u" [size]="26" />
                    <div style="font-size: 12.5px; font-weight: 500;">
                      {{ u.firstName }} {{ u.lastName }}
                    </div>
                  </div>
                }
              </td>
              <td>
                <span class="fd-mono-pill">{{ d.fp }}</span>
              </td>
              <td>
                <span class="fd-chip info" style="font-size: 11px;">
                  <faso-icon name="key" [size]="10" /> {{ d.type }}
                </span>
              </td>
              <td style="font-size: 12px; color: var(--text-2);">
                {{ d.ua }}
              </td>
              <td>
                <div class="fd-mono" style="font-size: 12px;">{{ d.ip }}</div>
                <div style="font-size: 11px; color: var(--text-3);">
                  {{ d.city }}
                </div>
              </td>
              <td style="font-size: 12.5px;">{{ d.lastUsed }}</td>
              <td>
                <span
                  class="fd-mono"
                  style="font-size: 12px; color: var(--text-2);"
                >
                  {{ d.trustedUntil }}
                </span>
              </td>
              <td>
                <button
                  class="fd-btn ghost sm"
                  style="color: var(--danger);"
                >
                  <faso-icon name="trash" [size]="12" />
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
export class DevicesPage {
  readonly lang = input<AdminLang>('fr');

  protected readonly devices = signal(MOCK_DEVICES);
  protected readonly users = signal(MOCK_USERS);

  protected userOf(device: TrustedDevice) {
    return this.users().find((u) => u.id === device.user);
  }
}
