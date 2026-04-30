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
import { FormsModule } from '@angular/forms';
import {
  MatDialog,
  MatDialogModule,
  MatDialogRef,
  MAT_DIALOG_DATA,
} from '@angular/material/dialog';
import { TranslateModule } from '@ngx-translate/core';
import {
  FasoIconComponent,
  FasoSettingRowComponent,
} from '../components-v2';
import type {
  AdminLang,
  AdminLevel,
  SettingHistoryEntry,
  SettingCategory,
} from '../models/admin.model';
import { MOCK_SETTINGS_HISTORY } from '../services/admin-mocks';

interface CatDef {
  readonly id: SettingCategory;
  readonly labelFr: string;
  readonly labelEn: string;
  readonly icon: 'shield' | 'key' | 'monitor' | 'flame' | 'log';
  readonly count: number;
}

// Le JSX source liste 6 catégories (pas 7) — `break_glass` est fusionné avec
// `grant` (libellé "Octroi & Break-Glass"). On respecte cette structure.
const SETTINGS_CATS: readonly CatDef[] = [
  { id: 'otp', labelFr: 'Politique OTP', labelEn: 'OTP Policy', icon: 'shield', count: 6 },
  { id: 'device_trust', labelFr: 'Device Trust', labelEn: 'Device Trust', icon: 'key', count: 6 },
  { id: 'session', labelFr: 'Sessions', labelEn: 'Sessions', icon: 'monitor', count: 5 },
  // 8 (legacy MFA + recovery) + 2 (Phase 4.b.6 risk thresholds — risk.score_threshold_step_up / risk.score_threshold_block).
  { id: 'mfa', labelFr: 'MFA & Recovery', labelEn: 'MFA & Recovery', icon: 'shield', count: 10 },
  { id: 'grant', labelFr: 'Octroi & Break-Glass', labelEn: 'Grants & Break-Glass', icon: 'flame', count: 8 },
  { id: 'audit', labelFr: 'Audit & rétention', labelEn: 'Audit & Retention', icon: 'log', count: 5 },
];

@Component({
  selector: 'faso-settings-history-dialog',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [CommonModule, MatDialogModule, FasoIconComponent],
  template: `
    <div class="fd-modal" style="width: 600px;">
      <div class="fd-modal-h">
        <div class="fd-h2">
          {{ data.lang === 'fr' ? 'Historique · ' : 'History · ' }}
          <span class="fd-mono" style="font-size: 14px;">
            {{ data.settingKey }}
          </span>
        </div>
        <div class="fd-help" style="margin-top: 4px;">
          {{
            data.lang === 'fr'
              ? data.history.length + ' versions · plus récente en premier'
              : data.history.length + ' versions · most recent first'
          }}
        </div>
      </div>
      <div class="fd-modal-b" style="max-height: 380px; overflow: auto;">
        @for (h of data.history; track h.v; let i = $index) {
          <div
            [style.display]="'flex'"
            [style.gap]="'14px'"
            [style.padding]="'12px 0'"
            [style.borderBottom]="
              i < data.history.length - 1
                ? '1px solid var(--border)'
                : 'none'
            "
          >
            <div style="width: 32px; text-align: center;">
              <div
                class="fd-mono"
                [style.background]="
                  i === 0 ? 'var(--primary)' : 'var(--surface-2)'
                "
                [style.color]="i === 0 ? '#fff' : 'var(--text-2)'"
                [style.borderRadius]="'6px'"
                [style.padding]="'4px 0'"
                [style.fontSize]="'11px'"
                [style.fontWeight]="600"
              >
                v{{ h.v }}
              </div>
            </div>
            <div style="flex: 1;">
              <div style="font-size: 12.5px; font-weight: 500;">
                {{ h.who }}
              </div>
              <div class="fd-help">
                {{ h.when }} ·
                <span class="fd-mono">{{ h.trace }}</span>
              </div>
              @if (h.motif) {
                <div
                  style="margin-top: 6px; font-size: 12.5px; color: var(--text-2); font-style: italic;"
                >
                  « {{ h.motif }} »
                </div>
              }
              <div style="margin-top: 6px; font-size: 11.5px;">
                @if (h.oldV !== null) {
                  <span
                    class="fd-mono"
                    style="background: var(--danger-soft); color: var(--danger); padding: 2px 6px; border-radius: 4px;"
                  >
                    {{ h.oldV }}s
                  </span>
                  <span style="margin: 0 6px; color: var(--text-3);">→</span>
                }
                <span
                  class="fd-mono"
                  style="background: var(--ok-soft); color: var(--ok); padding: 2px 6px; border-radius: 4px;"
                >
                  {{ h.newV }}s
                </span>
              </div>
            </div>
            @if (i !== 0) {
              <button
                class="fd-btn sm"
                (click)="restore(h)"
              >
                {{ data.lang === 'fr' ? 'Restaurer' : 'Restore' }}
              </button>
            }
          </div>
        }
      </div>
      <div class="fd-modal-f">
        <button class="fd-btn ghost" (click)="close()">
          {{ data.lang === 'fr' ? 'Fermer' : 'Close' }}
        </button>
      </div>
    </div>
  `,
})
export class SettingsHistoryDialog {
  protected readonly data = inject<{
    lang: AdminLang;
    settingKey: string;
    history: readonly SettingHistoryEntry[];
  }>(MAT_DIALOG_DATA);

  private readonly ref = inject(MatDialogRef<SettingsHistoryDialog>);

  protected close(): void {
    this.ref.close();
  }

  protected restore(h: SettingHistoryEntry): void {
    this.ref.close({ action: 'restore', version: h.v });
  }
}

@Component({
  selector: 'faso-settings-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    FormsModule,
    TranslateModule,
    MatDialogModule,
    FasoIconComponent,
    FasoSettingRowComponent,
  ],
  template: `
    <div class="fd-page-head">
      <div>
        <div class="fd-h1">
          {{
            lang() === 'fr'
              ? 'Paramètres de sécurité'
              : 'Security settings'
          }}
        </div>
        <div class="fd-page-sub">
          {{
            lang() === 'fr'
              ? '6 catégories · versionné en DB · publié sur Redpanda · rollback disponible. SUPER-ADMIN édite, ADMIN lit.'
              : '6 categories · DB-versioned · Redpanda-published · rollback available. SUPER-ADMIN edits, ADMIN reads.'
          }}
        </div>
      </div>
      @if (!canEdit()) {
        <span class="fd-chip warn">
          <faso-icon name="info" [size]="11" />
          {{
            lang() === 'fr' ? 'Lecture seule · ADMIN' : 'Read-only · ADMIN'
          }}
        </span>
      }
    </div>

    <div class="fd-card" style="overflow: hidden;">
      <div class="fd-settings-layout">
        <aside class="fd-settings-cats">
          @for (c of cats; track c.id) {
            <div
              class="fd-cat-item"
              [class.active]="cat() === c.id"
              (click)="cat.set(c.id)"
            >
              <span class="fd-row" style="gap: 8px;">
                <faso-icon [name]="c.icon" [size]="14" />
                <span>
                  {{ lang() === 'fr' ? c.labelFr : c.labelEn }}
                </span>
              </span>
              <span class="fd-cat-count">{{ c.count }}</span>
            </div>
          }
          <div class="fd-divider"></div>
          <div class="fd-help" style="padding: 0 8px;">
            {{
              lang() === 'fr'
                ? 'Cache BFF 30s · invalidation auto sur PUT'
                : 'BFF cache 30s · auto-invalidate on PUT'
            }}
          </div>
        </aside>

        <div style="padding: 20px 24px;">
          @switch (cat()) {
            @case ('otp') {
              <div
                style="display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 16px;"
              >
                <div>
                  <div class="fd-h2">
                    Politique OTP
                    <span
                      class="fd-mono"
                      style="color: var(--text-3); font-size: 13px; font-weight: 400;"
                    >
                      otp.*
                    </span>
                  </div>
                  <div class="fd-help" style="margin-top: 4px;">
                    {{
                      lang() === 'fr'
                        ? "Affecte tous les flows d'émission/vérification de codes à usage unique."
                        : 'Affects all OTP issue/verify flows.'
                    }}
                  </div>
                </div>
                <div class="fd-row">
                  <button
                    class="fd-btn ghost sm"
                    (click)="openHistory('otp.lifetime_seconds')"
                  >
                    <faso-icon name="clock" [size]="13" />
                    {{ lang() === 'fr' ? 'Historique' : 'History' }}
                  </button>
                  <button class="fd-btn ghost sm" [disabled]="!isDirty()">
                    {{ lang() === 'fr' ? 'Annuler' : 'Cancel' }}
                  </button>
                  <button
                    class="fd-btn primary sm"
                    [disabled]="!canEdit() || !isDirty()"
                  >
                    {{ lang() === 'fr' ? 'Sauvegarder' : 'Save' }}
                  </button>
                </div>
              </div>

              <faso-setting-row
                k="otp.lifetime_seconds"
                [label]="
                  lang() === 'fr' ? 'Durée de validité' : 'Lifetime'
                "
                [desc]="
                  lang() === 'fr'
                    ? 'Min 60s · max 900s · défaut 300s. Influe directement sur taux d’expiration en zone réseau lente.'
                    : 'Min 60s · max 900s · default 300s.'
                "
                [version]="4"
                updatedBy="Aminata Ouédraogo · 10:42"
                [dirty]="isDirty()"
              >
                <div class="fd-row">
                  <input
                    type="range"
                    class="fd-slider"
                    min="60"
                    max="900"
                    step="30"
                    [ngModel]="otpLifetime()"
                    (ngModelChange)="otpLifetime.set(+$event)"
                    [disabled]="!canEdit()"
                  />
                  <span
                    class="fd-mono"
                    style="min-width: 56px; text-align: right; font-size: 13px;"
                  >
                    {{ otpLifetime() }}s
                  </span>
                </div>
                <div class="fd-help">
                  {{ minutes() }} min {{ seconds() }}s ·
                  {{ lang() === 'fr' ? 'défaut' : 'default' }} 300s
                </div>
              </faso-setting-row>

              <faso-setting-row
                k="otp.max_attempts"
                [label]="
                  lang() === 'fr'
                    ? 'Tentatives max avant verrouillage'
                    : 'Max attempts before lock'
                "
                [desc]="
                  lang() === 'fr'
                    ? 'Au-delà, l’utilisateur est verrouillé pour la durée définie.'
                    : 'Beyond this, user is locked for defined duration.'
                "
                [version]="2"
                updatedBy="system · seed"
              >
                <div class="fd-row">
                  <button
                    class="fd-btn icon sm"
                    [disabled]="!canEdit()"
                    (click)="otpAttempts.set(max(1, otpAttempts() - 1))"
                  >
                    −
                  </button>
                  <input
                    class="fd-input"
                    style="width: 64px; text-align: center;"
                    [ngModel]="otpAttempts()"
                    (ngModelChange)="otpAttempts.set(+$event || 1)"
                    [disabled]="!canEdit()"
                  />
                  <button
                    class="fd-btn icon sm"
                    [disabled]="!canEdit()"
                    (click)="otpAttempts.set(min(10, otpAttempts() + 1))"
                  >
                    +
                  </button>
                </div>
              </faso-setting-row>

              <faso-setting-row
                k="otp.lock_duration_seconds"
                [label]="
                  lang() === 'fr' ? 'Durée de verrouillage' : 'Lock duration'
                "
                [desc]="
                  lang() === 'fr'
                    ? 'Empêche tout nouvel envoi OTP pour cet utilisateur.'
                    : 'Blocks new OTP sends for the user.'
                "
                [version]="1"
                updatedBy="system · seed"
              >
                <select
                  class="fd-select"
                  [disabled]="!canEdit()"
                  [ngModel]="otpLockDuration()"
                  (ngModelChange)="otpLockDuration.set(+$event)"
                >
                  <option [ngValue]="300">5 min</option>
                  <option [ngValue]="600">10 min</option>
                  <option [ngValue]="900">
                    15 min ({{
                      lang() === 'fr' ? 'défaut' : 'default'
                    }})
                  </option>
                  <option [ngValue]="1800">30 min</option>
                  <option [ngValue]="3600">1 h</option>
                </select>
              </faso-setting-row>

              <faso-setting-row
                k="otp.length"
                [label]="
                  lang() === 'fr'
                    ? 'Nombre de chiffres'
                    : 'Number of digits'
                "
                [desc]="
                  lang() === 'fr'
                    ? 'Augmenter améliore l’entropie mais alourdit la saisie mobile.'
                    : 'Higher = more entropy but harder mobile input.'
                "
                [version]="3"
                updatedBy="Souleymane Sawadogo · 12 mars"
              >
                <div class="fd-row" style="gap: 4px;">
                  @for (n of otpLengths; track n) {
                    <button
                      class="fd-btn sm"
                      [class.primary]="otpLength() === n"
                      (click)="canEdit() && otpLength.set(n)"
                      [disabled]="!canEdit()"
                    >
                      {{ n }}
                    </button>
                  }
                </div>
              </faso-setting-row>

              <faso-setting-row
                k="otp.rate_limit_per_user_5min"
                [label]="
                  lang() === 'fr'
                    ? 'OTP émis / utilisateur / 5 min'
                    : 'OTPs / user / 5 min'
                "
                [desc]="
                  lang() === 'fr'
                    ? 'Anti-abus · KAYA auth:otp:rl:&#123;userId&#125;.'
                    : 'Anti-abuse · KAYA auth:otp:rl:&#123;userId&#125;.'
                "
                [version]="1"
                updatedBy="system · seed"
              >
                <div class="fd-row">
                  <input
                    type="range"
                    class="fd-slider"
                    min="1"
                    max="10"
                    [ngModel]="otpRate()"
                    (ngModelChange)="otpRate.set(+$event)"
                    [disabled]="!canEdit()"
                  />
                  <span
                    class="fd-mono"
                    style="min-width: 56px; text-align: right;"
                  >
                    {{ otpRate() }}
                  </span>
                </div>
              </faso-setting-row>

              <faso-setting-row
                k="otp.allowed_methods"
                [label]="
                  lang() === 'fr' ? 'Canaux autorisés' : 'Allowed channels'
                "
                [desc]="
                  lang() === 'fr'
                    ? 'SMS désactivé tant que partenaire ONATEL n’est pas certifié ANSSI.'
                    : 'SMS disabled pending ANSSI certification of ONATEL provider.'
                "
                [version]="2"
                updatedBy="Aminata Ouédraogo · 04 fév"
              >
                <div class="fd-row" style="gap: 6px; flex-wrap: wrap;">
                  @for (m of methodKeys; track m.k) {
                    <span
                      class="fd-chip"
                      [class.role-admin]="methods()[m.k]"
                      [class.muted]="!methods()[m.k]"
                      [style.cursor]="canEdit() ? 'pointer' : 'default'"
                      style="font-size: 12px;"
                      (click)="canEdit() && toggleMethod(m.k)"
                    >
                      @if (methods()[m.k]) {
                        <faso-icon name="check" [size]="11" />
                      }
                      {{ m.label }}
                    </span>
                  }
                </div>
              </faso-setting-row>
            }

            @case ('device_trust') {
              <div class="fd-h2" style="margin-bottom: 14px;">
                Device Trust
                <span
                  class="fd-mono"
                  style="color: var(--text-3); font-size: 13px; font-weight: 400;"
                >
                  device_trust.*
                </span>
              </div>

              <faso-setting-row
                k="device_trust.enabled"
                [label]="
                  lang() === 'fr'
                    ? 'Activer le device-trust'
                    : 'Enable device-trust'
                "
                [desc]="
                  lang() === 'fr'
                    ? 'Désactiver imposera MFA à chaque connexion. Confirmation à motif obligatoire.'
                    : 'Disabling forces MFA every login. Confirmation with motive required.'
                "
                [version]="1"
                updatedBy="system · seed"
              >
                <div
                  class="fd-row"
                  style="justify-content: flex-end;"
                >
                  <span class="fd-toggle on"></span>
                </div>
              </faso-setting-row>

              <faso-setting-row
                k="device_trust.ttl_days"
                [label]="
                  lang() === 'fr'
                    ? 'TTL des appareils trustés'
                    : 'Trusted-device TTL'
                "
                [desc]="
                  lang() === 'fr'
                    ? 'Au terme, l’utilisateur doit re-MFA. Min 7j · max 90j.'
                    : 'After expiry, user must re-MFA. Min 7d · max 90d.'
                "
                [version]="2"
                updatedBy="Aminata · 12 mars"
              >
                <div class="fd-row">
                  <input
                    type="range"
                    class="fd-slider"
                    min="7"
                    max="90"
                    [ngModel]="devTtl()"
                    (ngModelChange)="devTtl.set(+$event)"
                    [disabled]="!canEdit()"
                  />
                  <span
                    class="fd-mono"
                    style="min-width: 56px; text-align: right;"
                  >
                    {{ devTtl() }} j
                  </span>
                </div>
              </faso-setting-row>

              <faso-setting-row
                k="device_trust.max_per_user"
                [label]="
                  lang() === 'fr'
                    ? 'Appareils max par utilisateur'
                    : 'Max devices per user'
                "
                desc=""
                [version]="1"
                updatedBy="system · seed"
              >
                <select
                  class="fd-select"
                  [disabled]="!canEdit()"
                  [ngModel]="devMax()"
                  (ngModelChange)="devMax.set(+$event)"
                >
                  <option [ngValue]="3">3</option>
                  <option [ngValue]="5">5</option>
                  <option [ngValue]="10">10</option>
                  <option [ngValue]="20">20</option>
                </select>
              </faso-setting-row>

              <faso-setting-row
                k="device_trust.fingerprint_strictness"
                [label]="
                  lang() === 'fr'
                    ? 'Stricteté de l’empreinte'
                    : 'Fingerprint strictness'
                "
                [desc]="
                  'low=UA · medium=UA+IP/24+lang · high=+TLS-fingerprint'
                "
                [version]="1"
                updatedBy="system · seed"
              >
                <div class="fd-row" style="gap: 4px;">
                  @for (s of strictnessLevels; track s) {
                    <button
                      class="fd-btn sm"
                      [class.primary]="strictness() === s"
                      [disabled]="!canEdit()"
                      (click)="strictness.set(s)"
                    >
                      {{ s }}
                    </button>
                  }
                </div>
              </faso-setting-row>

              <faso-setting-row
                k="device_trust.re_verify_on_ip_change"
                [label]="
                  lang() === 'fr'
                    ? 'Re-vérifier au changement d’IP'
                    : 'Re-verify on IP change'
                "
                [desc]="
                  lang() === 'fr'
                    ? 'Recommandé pour les rôles SUPER-ADMIN.'
                    : 'Recommended for SUPER-ADMIN roles.'
                "
                [version]="1"
                updatedBy="system · seed"
              >
                <div
                  class="fd-row"
                  style="justify-content: flex-end;"
                >
                  <span
                    class="fd-toggle"
                    [class.on]="reverifyOnIp()"
                    (click)="canEdit() && reverifyOnIp.set(!reverifyOnIp())"
                  ></span>
                </div>
              </faso-setting-row>

              <faso-setting-row
                k="device_trust.auto_revoke_on_password_change"
                [label]="
                  lang() === 'fr'
                    ? 'Révoquer auto sur changement de mot de passe'
                    : 'Auto-revoke on password change'
                "
                desc=""
                [version]="1"
                updatedBy="system · seed"
              >
                <div
                  class="fd-row"
                  style="justify-content: flex-end;"
                >
                  <span class="fd-toggle on"></span>
                </div>
              </faso-setting-row>
            }

            @default {
              <div class="fd-h2" style="margin-bottom: 14px;">
                {{ catLabel() }}
              </div>
              <div
                class="fd-help"
                style="padding: 24px; text-align: center; background: var(--surface-2); border-radius: var(--r-md);"
              >
                {{
                  lang() === 'fr'
                    ? 'Catégorie · structure identique à OTP / Device Trust. Mêmes patterns SettingRow + history + revert.'
                    : 'Category · same structure as OTP / Device Trust. Same SettingRow + history + revert patterns.'
                }}
              </div>
            }
          }
        </div>
      </div>
    </div>
  `,
  styles: [`:host { display: contents; }`],
})
export class SettingsPage {
  readonly lang = input<AdminLang>('fr');
  readonly role = input<AdminLevel>('ADMIN');

  protected readonly cats = SETTINGS_CATS;
  protected readonly cat = signal<SettingCategory>('otp');

  protected readonly otpLifetime = signal<number>(600);
  protected readonly otpAttempts = signal<number>(3);
  protected readonly otpLength = signal<number>(8);
  protected readonly otpLockDuration = signal<number>(900);
  protected readonly otpRate = signal<number>(3);
  protected readonly methods = signal<{ email: boolean; totp: boolean; sms: boolean }>({
    email: true,
    totp: true,
    sms: false,
  });

  protected readonly devTtl = signal<number>(30);
  protected readonly devMax = signal<number>(5);
  protected readonly strictness = signal<'low' | 'medium' | 'high'>('medium');
  protected readonly reverifyOnIp = signal<boolean>(false);

  protected readonly otpLengths = [6, 7, 8, 9, 10] as const;
  protected readonly strictnessLevels = ['low', 'medium', 'high'] as const;
  protected readonly methodKeys = [
    { k: 'email' as const, label: 'E-mail' },
    { k: 'totp' as const, label: 'TOTP' },
    { k: 'sms' as const, label: 'SMS' },
  ];

  protected readonly canEdit = computed(() => this.role() === 'SUPER-ADMIN');
  protected readonly isDirty = computed(() => this.otpLifetime() !== 300);

  protected readonly minutes = computed(() => Math.floor(this.otpLifetime() / 60));
  protected readonly seconds = computed(() => this.otpLifetime() % 60);

  protected readonly catLabel = computed(() => {
    const c = SETTINGS_CATS.find((x) => x.id === this.cat());
    if (!c) return '';
    return this.lang() === 'fr' ? c.labelFr : c.labelEn;
  });

  private readonly dialog = inject(MatDialog);

  protected toggleMethod(k: 'email' | 'totp' | 'sms'): void {
    this.methods.update((m) => ({ ...m, [k]: !m[k] }));
  }

  protected openHistory(settingKey: string): void {
    this.dialog.open(SettingsHistoryDialog, {
      data: {
        lang: this.lang(),
        settingKey,
        history: MOCK_SETTINGS_HISTORY,
      },
      panelClass: 'fd-dialog-panel',
      backdropClass: 'fd-dialog-backdrop',
    });
  }

  protected min(a: number, b: number): number {
    return Math.min(a, b);
  }

  protected max(a: number, b: number): number {
    return Math.max(a, b);
  }
}
