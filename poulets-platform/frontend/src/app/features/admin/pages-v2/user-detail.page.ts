// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { CommonModule } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  input,
  signal,
} from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';
import { toSignal } from '@angular/core/rxjs-interop';
import { MatDialog, MatDialogModule } from '@angular/material/dialog';
import { TranslateModule } from '@ngx-translate/core';
import {
  FasoAvatarComponent,
  FasoIconComponent,
  FasoRoleChipComponent,
} from '../components-v2';
import { AdminLang, AdminLevel } from '../models/admin.model';
import { MOCK_AUDIT, MOCK_DEVICES, MOCK_USERS } from '../services/admin-mocks';
import {
  GrantRoleStepperDialog,
  type GrantRoleDialogData,
} from './grant-role-stepper.dialog';

@Component({
  selector: 'faso-user-detail-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    MatDialogModule,
    TranslateModule,
    FasoIconComponent,
    FasoAvatarComponent,
    FasoRoleChipComponent,
  ],
  template: `
    @if (user(); as u) {
      <div class="fd-page-head">
        <div class="fd-row" style="gap: 14px;">
          <button class="fd-btn ghost icon" (click)="goBack()">
            <faso-icon
              name="chevR"
              [size]="14"
              style="transform: rotate(180deg);"
            />
          </button>
          <faso-avatar [user]="u" [size]="48" />
          <div>
            <div class="fd-h1">
              {{ u.firstName }} {{ u.lastName }}
              <faso-role-chip [role]="u.role" />
            </div>
            <div class="fd-page-sub">
              <span class="fd-mono">{{ u.email }}</span> · {{ u.department }} ·
              {{ lang() === 'fr' ? 'créé' : 'created' }} {{ u.createdAt }}
            </div>
          </div>
        </div>
        <div class="fd-row">
          <button class="fd-btn">
            <faso-icon name="rotate" [size]="13" />
            {{ lang() === 'fr' ? 'Réinit. MFA' : 'Reset MFA' }}
          </button>
          <button class="fd-btn">
            <faso-icon name="logout" [size]="13" />
            {{ lang() === 'fr' ? 'Forcer logout' : 'Force logout' }}
          </button>
          <button class="fd-btn danger">
            {{ lang() === 'fr' ? 'Suspendre' : 'Suspend' }}
          </button>
        </div>
      </div>

      <div style="display: grid; grid-template-columns: 1.6fr 1fr; gap: 16px;">
        <div style="display: flex; flex-direction: column; gap: 16px;">
          <div class="fd-card">
            <div class="fd-card-h">
              <div class="fd-card-h-title">
                {{
                  lang() === 'fr' ? 'Rôles & permissions' : 'Roles & permissions'
                }}
              </div>
              <button class="fd-btn sm">
                {{ lang() === 'fr' ? 'Octroyer un rôle' : 'Grant role' }}
              </button>
            </div>
            <div class="fd-card-b">
              <div class="fd-row" style="gap: 8px; flex-wrap: wrap;">
                <span class="fd-chip role-admin" style="font-size: 12px;">
                  ADMIN
                  <span style="opacity: 0.6; margin-left: 4px;">
                    · tenant=état-civil
                  </span>
                  <faso-icon name="x" [size]="11" />
                </span>
                <span class="fd-chip role-manager" style="font-size: 12px;">
                  MANAGER
                  <span style="opacity: 0.6; margin-left: 4px;">
                    · scope=DIRECTION
                  </span>
                  <faso-icon name="x" [size]="11" />
                </span>
              </div>
              <div class="fd-divider"></div>
              <div style="font-size: 12px; color: var(--text-3);">
                {{ lang() === 'fr' ? 'Octroyé par' : 'Granted by' }}
                <strong style="color: var(--text-2);">Aminata Ouédraogo</strong>
                · 18 juin 2024 ·
                <span class="fd-mono-pill">8a1d3f47</span>
              </div>
            </div>
          </div>

          <div class="fd-card">
            <div class="fd-card-h">
              <div class="fd-card-h-title">
                {{
                  lang() === 'fr'
                    ? 'Capacités effectives'
                    : 'Effective capabilities'
                }}
                <span style="color: var(--text-3); font-weight: 400;">
                  · {{ effectiveCapabilities().length }}
                </span>
              </div>
              @if (canModifyCaps()) {
                <button
                  class="fd-btn sm"
                  type="button"
                  (click)="openEditCapabilities()"
                >
                  {{
                    lang() === 'fr'
                      ? 'Modifier les capacités'
                      : 'Modify capabilities'
                  }}
                </button>
              }
            </div>
            <div
              class="fd-card-b"
              style="display: flex; flex-direction: column; gap: 10px;"
            >
              @for (group of capabilitiesByDomain(); track group.domain) {
                <div>
                  <div
                    style="font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600; color: var(--text-3); margin-bottom: 4px;"
                  >
                    {{
                      lang() === 'fr'
                        ? domainLabelFr[group.domain]
                        : domainLabelEn[group.domain]
                    }}
                  </div>
                  <div style="display: flex; flex-wrap: wrap; gap: 4px;">
                    @for (cap of group.caps; track cap) {
                      <span
                        class="fd-chip role-admin"
                        style="font-size: 11px; gap: 4px;"
                      >
                        <faso-icon name="check" [size]="10" />
                        <span class="fd-mono">{{ cap }}</span>
                      </span>
                    }
                  </div>
                </div>
              }
              @if (effectiveCapabilities().length === 0) {
                <div class="fd-help">
                  {{
                    lang() === 'fr'
                      ? 'Aucune capacité spécifique octroyée.'
                      : 'No specific capability granted.'
                  }}
                </div>
              }
            </div>
          </div>

          <div class="fd-card">
            <div class="fd-card-h">
              <div class="fd-card-h-title">
                {{ lang() === 'fr' ? 'Sessions actives' : 'Active sessions' }}
                <span style="color: var(--text-3); font-weight: 400;">· 2</span>
              </div>
            </div>
            <table class="fd-table">
              <thead>
                <tr>
                  <th>{{ lang() === 'fr' ? 'Appareil' : 'Device' }}</th>
                  <th>IP</th>
                  <th>{{ lang() === 'fr' ? 'Activité' : 'Activity' }}</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td>
                    <div style="font-weight: 500; font-size: 12.5px;">
                      Dell Latitude 7440
                    </div>
                    <div style="font-size: 11px; color: var(--text-3);">
                      Firefox 124 · Ubuntu 22.04
                    </div>
                  </td>
                  <td>
                    <span class="fd-mono">196.28.111.18</span>
                    <div style="font-size: 11px; color: var(--text-3);">
                      Ouagadougou
                    </div>
                  </td>
                  <td style="font-size: 12.5px;">
                    {{ lang() === 'fr' ? 'il y a 1 h' : '1h ago' }}
                  </td>
                  <td>
                    <button
                      class="fd-btn ghost sm danger"
                      style="color: var(--danger);"
                    >
                      {{ lang() === 'fr' ? 'Révoquer' : 'Revoke' }}
                    </button>
                  </td>
                </tr>
                <tr>
                  <td>
                    <div style="font-weight: 500; font-size: 12.5px;">
                      iPhone 15 Pro
                    </div>
                    <div style="font-size: 11px; color: var(--text-3);">
                      Safari Mobile · iOS 17
                    </div>
                  </td>
                  <td>
                    <span class="fd-mono">41.207.99.4</span>
                    <div style="font-size: 11px; color: var(--text-3);">
                      Ouagadougou
                    </div>
                  </td>
                  <td style="font-size: 12.5px;">
                    {{ lang() === 'fr' ? 'il y a 4 h' : '4h ago' }}
                  </td>
                  <td>
                    <button
                      class="fd-btn ghost sm"
                      style="color: var(--danger);"
                    >
                      {{ lang() === 'fr' ? 'Révoquer' : 'Revoke' }}
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <div class="fd-card">
            <div class="fd-card-h">
              <div class="fd-card-h-title">
                {{ lang() === 'fr' ? 'Historique audit' : 'Audit history' }}
              </div>
              <button class="fd-btn ghost sm">
                {{ lang() === 'fr' ? 'Voir tout' : 'View all' }}
                <faso-icon name="chevR" [size]="12" />
              </button>
            </div>
            <div class="fd-card-b">
              @for (a of recentAudit(); track a.id) {
                <div class="fd-tl-item">
                  <span
                    class="fd-tl-dot"
                    [style.background]="
                      a.critical ? 'var(--danger)' : 'var(--primary)'
                    "
                  ></span>
                  <div>
                    <div
                      class="fd-tl-title fd-mono"
                      style="font-size: 12.5px;"
                    >
                      {{ a.action }}
                    </div>
                    <div class="fd-tl-meta">
                      {{ a.desc }}
                      <span class="fd-mono-pill">trace · {{ a.traceId }}</span>
                    </div>
                  </div>
                  <div class="fd-tl-time">
                    {{ a.time }} · {{ a.date.split(' ')[0] }}
                    {{ a.date.split(' ')[1].slice(0, 3) }}
                  </div>
                </div>
              }
            </div>
          </div>
        </div>

        <div style="display: flex; flex-direction: column; gap: 16px;">
          <div class="fd-card">
            <div class="fd-card-h">
              <div class="fd-card-h-title">MFA</div>
            </div>
            <div
              class="fd-card-b"
              style="display: flex; flex-direction: column; gap: 12px;"
            >
              <div
                style="display: flex; align-items: center; gap: 12px; padding: 8px 0;"
              >
                <div
                  style="width: 36px; height: 36px; border-radius: 8px; background: var(--primary-soft); display: flex; align-items: center; justify-content: center; color: var(--primary);"
                >
                  <faso-icon name="key" [size]="18" />
                </div>
                <div style="flex: 1;">
                  <div style="font-weight: 500; font-size: 13px;">
                    PassKey · YubiKey 5C
                  </div>
                  <div style="font-size: 11px; color: var(--text-3);">
                    {{
                      lang() === 'fr'
                        ? 'Utilisée il y a 1 h'
                        : 'Used 1h ago'
                    }}
                  </div>
                </div>
                <span class="fd-chip ok">
                  {{ lang() === 'fr' ? 'Actif' : 'Active' }}
                </span>
              </div>
              <div
                style="display: flex; align-items: center; gap: 12px; padding: 8px 0; border-top: 1px solid var(--border);"
              >
                <div
                  style="width: 36px; height: 36px; border-radius: 8px; background: var(--info-soft); display: flex; align-items: center; justify-content: center; color: var(--info);"
                >
                  <faso-icon name="qr" [size]="18" />
                </div>
                <div style="flex: 1;">
                  <div style="font-weight: 500; font-size: 13px;">
                    TOTP · Authy
                  </div>
                  <div style="font-size: 11px; color: var(--text-3);">
                    {{ lang() === 'fr' ? 'Désactivée' : 'Disabled' }} · 12 mars
                  </div>
                </div>
                <span class="fd-chip muted">—</span>
              </div>
              <div
                style="display: flex; align-items: center; gap: 12px; padding: 8px 0; border-top: 1px solid var(--border);"
              >
                <div
                  style="width: 36px; height: 36px; border-radius: 8px; background: var(--accent-soft); display: flex; align-items: center; justify-content: center; color: var(--accent);"
                >
                  <faso-icon name="shield" [size]="18" />
                </div>
                <div style="flex: 1;">
                  <div style="font-weight: 500; font-size: 13px;">
                    {{
                      lang() === 'fr'
                        ? 'Codes de récupération'
                        : 'Backup codes'
                    }}
                  </div>
                  <div style="font-size: 11px; color: var(--text-3);">
                    8 / 10
                    {{
                      lang() === 'fr'
                        ? 'restants · expire 12/03/2027'
                        : 'remaining · expires 12/03/2027'
                    }}
                  </div>
                </div>
                <button class="fd-btn ghost sm">
                  {{ lang() === 'fr' ? 'Régénérer' : 'Regenerate' }}
                </button>
              </div>
            </div>
          </div>

          <div class="fd-card">
            <div class="fd-card-h">
              <div class="fd-card-h-title">
                {{
                  lang() === 'fr' ? 'Appareils trustés' : 'Trusted devices'
                }}
                <span style="color: var(--text-3); font-weight: 400;">
                  · {{ userDevices().length }}
                </span>
              </div>
            </div>
            <div
              class="fd-card-b"
              style="display: flex; flex-direction: column; gap: 12px;"
            >
              @for (d of userDevices(); track d.id) {
                <div
                  style="padding: 8px 0; border-bottom: 1px solid var(--border);"
                >
                  <div
                    style="display: flex; justify-content: space-between; align-items: flex-start;"
                  >
                    <div>
                      <div style="font-weight: 500; font-size: 13px;">
                        {{ d.type }}
                      </div>
                      <div
                        class="fd-mono"
                        style="font-size: 11px; color: var(--text-3); margin-top: 2px;"
                      >
                        {{ d.fp }}
                      </div>
                      <div
                        style="font-size: 11px; color: var(--text-3); margin-top: 4px;"
                      >
                        {{ d.ua }} · {{ d.city }}
                      </div>
                    </div>
                    <button
                      class="fd-btn ghost sm"
                      style="color: var(--danger);"
                    >
                      <faso-icon name="trash" [size]="12" />
                    </button>
                  </div>
                </div>
              }
            </div>
          </div>

          <div class="fd-card">
            <div class="fd-card-b">
              <div
                style="font-size: 11px; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600;"
              >
                {{
                  lang() === 'fr'
                    ? 'Profil · lecture seule'
                    : 'Profile · read-only'
                }}
              </div>
              <div class="fd-divider"></div>
              <div
                style="display: grid; grid-template-columns: auto 1fr; gap: 6px 16px; font-size: 12.5px;"
              >
                <span style="color: var(--text-3);">ID Kratos</span>
                <span class="fd-mono">{{ u.id }}-7f2ea8</span>
                <span style="color: var(--text-3);">
                  {{ lang() === 'fr' ? 'Téléphone' : 'Phone' }}
                </span>
                <span>+226 70 12 34 56</span>
                <span style="color: var(--text-3);">Tenant</span>
                <span>etat-civil-ougadougou</span>
                <span style="color: var(--text-3);">
                  {{ lang() === 'fr' ? 'Échecs login' : 'Failed logins' }}
                </span>
                <span>{{ u.failedLogins }} / 5</span>
                <span style="color: var(--text-3);">
                  {{ lang() === 'fr' ? 'Statut Keto' : 'Keto status' }}
                </span>
                <span>
                  <span class="fd-dot ok"></span>
                  {{
                    lang() === 'fr' ? 'synchro · 23s' : 'synced · 23s'
                  }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    }
  `,
  styles: [
    `
      :host {
        display: contents;
      }
    `,
  ],
})
export class UserDetailPage {
  readonly lang = input<AdminLang>('fr');
  /** Rôle de l'acteur courant — détermine la visibilité du bouton "Modifier les capacités". */
  readonly actorRole = input<AdminLevel>('SUPER-ADMIN');

  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);
  private readonly dialog = inject(MatDialog);

  protected readonly users = signal(MOCK_USERS);
  protected readonly devices = signal(MOCK_DEVICES);
  protected readonly audit = signal(MOCK_AUDIT);

  private readonly params = toSignal(this.route.paramMap, {
    initialValue: this.route.snapshot.paramMap,
  });

  protected readonly user = computed(() => {
    const id = this.params().get('userId');
    const found = id ? this.users().find((u) => u.id === id) : undefined;
    return found ?? this.users()[2];
  });

  protected readonly userDevices = computed(() => {
    const u = this.user();
    return u ? this.devices().filter((d) => d.user === u.id) : [];
  });

  protected readonly recentAudit = computed(() => this.audit().slice(0, 4));

  /**
   * Stub des capacités effectives — sera remplacé par GET /api/admin/users/{id}
   * étendu avec `capabilities: string[]`.
   */
  protected readonly effectiveCapabilities = signal<readonly string[]>([
    'users:invite',
    'users:suspend',
    'users:view_all',
    'sessions:list',
    'sessions:revoke',
    'audit:view',
    'audit:export',
    'mfa:reset',
    'devices:list',
  ]);

  protected readonly capabilitiesByDomain = computed(() => {
    const groups: Record<string, string[]> = {};
    for (const cap of this.effectiveCapabilities()) {
      const domain = cap.split(':')[0] ?? 'misc';
      (groups[domain] ??= []).push(cap);
    }
    const order = [
      'users',
      'sessions',
      'devices',
      'mfa',
      'audit',
      'settings',
      'break_glass',
      'recovery',
      'roles',
    ];
    return order
      .filter((d) => groups[d])
      .map((domain) => ({ domain, caps: groups[domain] ?? [] }));
  });

  protected readonly domainLabelFr: Record<string, string> = {
    users: 'Utilisateurs',
    sessions: 'Sessions',
    devices: 'Appareils',
    mfa: 'MFA',
    audit: 'Audit',
    settings: 'Paramètres',
    break_glass: 'Break-Glass',
    recovery: 'Récupération',
    roles: 'Rôles',
  };

  protected readonly domainLabelEn: Record<string, string> = {
    users: 'Users',
    sessions: 'Sessions',
    devices: 'Devices',
    mfa: 'MFA',
    audit: 'Audit',
    settings: 'Settings',
    break_glass: 'Break-Glass',
    recovery: 'Recovery',
    roles: 'Roles',
  };

  /** Visible si l'acteur peut octroyer des rôles (proxy pour roles:grant_*). */
  protected readonly canModifyCaps = computed(
    () => this.actorRole() === 'SUPER-ADMIN' || this.actorRole() === 'ADMIN',
  );

  protected goBack(): void {
    void this.router.navigate(['..'], { relativeTo: this.route });
  }

  protected openEditCapabilities(): void {
    const u = this.user();
    if (!u) return;
    const data: GrantRoleDialogData = {
      target: u,
      actorRole: this.actorRole(),
      lang: this.lang(),
      editCapsOnly: true,
      initialCapabilities: this.effectiveCapabilities(),
    };
    this.dialog.open(GrantRoleStepperDialog, { data, autoFocus: false });
  }
}
