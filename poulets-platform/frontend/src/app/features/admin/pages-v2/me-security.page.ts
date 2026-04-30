// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso
//
// Page self-management /admin/me/security — SUPER-ADMIN et inférieurs gèrent
// leurs propres facteurs sans intervention tierce. Cf. DELTA-REQUIREMENTS-2026-04-30 §3.

import { CommonModule } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  input,
  signal,
} from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import {
  FormBuilder,
  ReactiveFormsModule,
  Validators,
  type FormGroup,
} from '@angular/forms';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import {
  FasoIconComponent,
  FasoOtpInputComponent,
} from '../components-v2';
import type { AdminLang } from '../models/admin.model';

interface PasskeySummary {
  readonly id: string;
  readonly label: string;
  readonly addedAt: string;
}

interface SessionSummary {
  readonly id: string;
  readonly device: string;
  readonly ip: string;
  readonly lastActive: string;
  readonly current: boolean;
}

@Component({
  selector: 'faso-me-security-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    ReactiveFormsModule,
    RouterLink,
    TranslateModule,
    FasoIconComponent,
    FasoOtpInputComponent,
  ],
  template: `
    <div class="fd-page-head">
      <div>
        <div class="fd-h1">
          {{ lang() === 'fr' ? 'Sécurité de mon compte' : 'My account security' }}
        </div>
        <div class="fd-page-sub">
          {{
            lang() === 'fr'
              ? "Gérez vos facteurs d'authentification. Aucune intervention tierce requise pour SUPER-ADMIN."
              : 'Manage your authentication factors. No third-party intervention required for SUPER-ADMIN.'
          }}
        </div>
      </div>
    </div>

    @if (forceReenroll()) {
      <div class="fd-banner warn" style="margin-bottom: 16px;">
        <faso-icon name="alertTri" [size]="16" />
        <div class="fd-banner-body">
          <strong>
            {{
              lang() === 'fr'
                ? 'Réenrôlement MFA obligatoire'
                : 'MFA re-enrollment required'
            }}
          </strong>
          ·
          {{
            lang() === 'fr'
              ? 'Une récupération a été initiée. Réenrôlez un facteur (PassKey ou TOTP) avant de poursuivre.'
              : 'A recovery was initiated. Re-enroll a factor (PassKey or TOTP) before continuing.'
          }}
        </div>
      </div>
    }

    <div class="me-grid">
      <!-- Card Mot de passe -->
      <div class="fd-card">
        <div class="fd-card-h">
          <div class="fd-card-h-title">
            <faso-icon name="key" [size]="14" />
            {{ lang() === 'fr' ? 'Mot de passe' : 'Password' }}
          </div>
        </div>
        <div class="fd-card-b">
          <form
            [formGroup]="passwordForm"
            (ngSubmit)="onChangePassword()"
            style="display: flex; flex-direction: column; gap: 10px;"
          >
            <label class="fd-help">
              {{ lang() === 'fr' ? 'Mot de passe actuel' : 'Current password' }}
            </label>
            <input
              type="password"
              class="fd-input"
              formControlName="currentPassword"
              autocomplete="current-password"
            />

            <label class="fd-help">
              {{
                lang() === 'fr'
                  ? 'Nouveau mot de passe (≥ 12 caractères)'
                  : 'New password (≥ 12 characters)'
              }}
            </label>
            <input
              type="password"
              class="fd-input"
              formControlName="newPassword"
              autocomplete="new-password"
            />

            <label class="fd-help">
              {{ lang() === 'fr' ? 'Confirmer' : 'Confirm' }}
            </label>
            <input
              type="password"
              class="fd-input"
              formControlName="confirmPassword"
              autocomplete="new-password"
            />

            @if (passwordError()) {
              <div class="fd-help" style="color: var(--danger);">
                {{ passwordError() }}
              </div>
            }

            @if (passwordChanged()) {
              <div class="fd-help" style="color: var(--ok);">
                <faso-icon name="check" [size]="12" />
                {{
                  lang() === 'fr'
                    ? 'Mot de passe mis à jour.'
                    : 'Password updated.'
                }}
              </div>
            }

            <button
              type="submit"
              class="fd-btn primary"
              [disabled]="passwordForm.invalid"
            >
              {{
                lang() === 'fr' ? 'Changer le mot de passe' : 'Change password'
              }}
            </button>
            <div class="fd-help">
              POST <span class="fd-mono">/api/admin/me/password</span>
            </div>
          </form>
        </div>
      </div>

      <!-- Card PassKey -->
      <div class="fd-card">
        <div class="fd-card-h">
          <div class="fd-card-h-title">
            <faso-icon name="fp" [size]="14" />
            {{ lang() === 'fr' ? 'PassKey' : 'PassKey' }}
            <span style="color: var(--text-3); font-weight: 400;">
              · {{ passkeys().length }}
            </span>
          </div>
        </div>
        <div
          class="fd-card-b"
          style="display: flex; flex-direction: column; gap: 10px;"
        >
          @for (pk of passkeys(); track pk.id) {
            <div
              style="padding: 10px 12px; border: 1px solid var(--border); border-radius: var(--r-sm); display: flex; align-items: center; gap: 10px;"
            >
              <div
                style="width: 32px; height: 32px; border-radius: 8px; background: var(--primary); color: #fff; display: flex; align-items: center; justify-content: center;"
              >
                <faso-icon name="key" [size]="15" />
              </div>
              <div style="flex: 1; min-width: 0;">
                <div style="font-weight: 500; font-size: 13px;">
                  {{ pk.label }}
                </div>
                <div style="font-size: 11px; color: var(--text-3);">
                  {{ pk.addedAt }}
                </div>
              </div>
              <button
                class="fd-btn ghost sm"
                style="color: var(--danger);"
                (click)="onRemovePasskey(pk.id)"
                type="button"
              >
                <faso-icon name="trash" [size]="12" />
              </button>
            </div>
          }
          <button
            type="button"
            class="fd-btn primary"
            (click)="onEnrollPasskey()"
          >
            <faso-icon name="plus" [size]="13" />
            {{ lang() === 'fr' ? 'Ajouter PassKey' : 'Add PassKey' }}
          </button>
          <div class="fd-help">
            <span class="fd-mono">@simplewebauthn/browser</span>
            ·
            POST <span class="fd-mono">/api/admin/me/passkeys/enroll/{begin,finish}</span>
          </div>
        </div>
      </div>

      <!-- Card TOTP -->
      <div class="fd-card">
        <div class="fd-card-h">
          <div class="fd-card-h-title">
            <faso-icon name="qr" [size]="14" />
            TOTP
          </div>
          @if (totpEnrolled()) {
            <span class="fd-chip ok" style="font-size: 11px;">
              {{ lang() === 'fr' ? 'Activé' : 'Enabled' }}
            </span>
          } @else {
            <span class="fd-chip muted" style="font-size: 11px;">—</span>
          }
        </div>
        <div class="fd-card-b">
          @if (!totpEnrolled() && !totpStepperOpen()) {
            <div class="fd-help" style="margin-bottom: 10px;">
              {{
                lang() === 'fr'
                  ? "Aucune application TOTP enrôlée. Activez pour générer un QR code."
                  : 'No TOTP app enrolled. Enable to generate a QR code.'
              }}
            </div>
            <button
              class="fd-btn primary"
              type="button"
              (click)="totpStepperOpen.set(true)"
            >
              {{ lang() === 'fr' ? 'Activer TOTP' : 'Enable TOTP' }}
            </button>
          }

          @if (totpEnrolled()) {
            <div class="fd-help" style="margin-bottom: 10px;">
              {{
                lang() === 'fr'
                  ? 'Application TOTP enrôlée. Désactiver supprime le secret.'
                  : 'TOTP app enrolled. Disabling removes the secret.'
              }}
            </div>
            <button
              class="fd-btn danger"
              type="button"
              (click)="onDisableTotp()"
            >
              {{ lang() === 'fr' ? 'Désactiver TOTP' : 'Disable TOTP' }}
            </button>
          }

          @if (totpStepperOpen() && !totpEnrolled()) {
            <div
              style="padding: 12px; background: var(--surface-2); border: 1px solid var(--border); border-radius: var(--r-sm); margin-bottom: 12px;"
            >
              <div style="font-weight: 600; font-size: 12.5px; margin-bottom: 6px;">
                {{
                  lang() === 'fr'
                    ? 'Étape 1 · Scanner avec votre app TOTP'
                    : 'Step 1 · Scan with your TOTP app'
                }}
              </div>
              <div
                class="fd-mono"
                style="word-break: break-all; font-size: 11px; padding: 8px; background: var(--surface); border-radius: var(--r-sm);"
              >
                {{ totpSecret }}
              </div>
            </div>
            <div style="font-weight: 600; font-size: 12.5px; margin-bottom: 8px;">
              {{
                lang() === 'fr'
                  ? 'Étape 2 · Confirmer 6 chiffres'
                  : 'Step 2 · Confirm 6 digits'
              }}
            </div>
            <faso-otp-input
              [length]="6"
              [(value)]="totpCode"
              (complete)="onTotpVerify($event)"
            />
            <div style="display: flex; gap: 8px; margin-top: 10px;">
              <button
                class="fd-btn primary"
                type="button"
                [disabled]="totpCode().length !== 6"
                (click)="onTotpFinish()"
              >
                {{ lang() === 'fr' ? 'Confirmer' : 'Confirm' }}
              </button>
              <button
                class="fd-btn ghost"
                type="button"
                (click)="totpStepperOpen.set(false); totpCode.set('')"
              >
                {{ lang() === 'fr' ? 'Annuler' : 'Cancel' }}
              </button>
            </div>
          }
          <div class="fd-help" style="margin-top: 10px;">
            POST <span class="fd-mono">/api/admin/me/totp/enroll/{begin,finish}</span>
          </div>
        </div>
      </div>

      <!-- Card Codes de récupération -->
      <div class="fd-card">
        <div class="fd-card-h">
          <div class="fd-card-h-title">
            <faso-icon name="shield" [size]="14" />
            {{
              lang() === 'fr' ? 'Codes de récupération' : 'Recovery codes'
            }}
          </div>
          <span style="font-size: 12px; color: var(--text-3);">
            {{ recoveryRemaining() }} / {{ recoveryTotal }}
            {{ lang() === 'fr' ? 'restants' : 'remaining' }}
          </span>
        </div>
        <div class="fd-card-b">
          <div class="fd-help" style="margin-bottom: 10px;">
            {{
              lang() === 'fr'
                ? 'Régénérer invalide tous les codes existants.'
                : 'Regenerating invalidates all existing codes.'
            }}
          </div>

          @if (regeneratedCodes().length > 0) {
            <div
              class="fd-banner warn"
              style="margin-bottom: 10px; padding: 8px 10px; font-size: 12px;"
            >
              <faso-icon name="alertTri" [size]="13" />
              <div class="fd-banner-body">
                {{
                  lang() === 'fr'
                    ? 'Téléchargez immédiatement, ces codes ne seront plus visibles.'
                    : 'Download now — these codes will not be shown again.'
                }}
              </div>
            </div>
            <div
              style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 6px; margin-bottom: 10px;"
            >
              @for (c of regeneratedCodes(); track c) {
                <div
                  class="fd-mono"
                  style="padding: 6px 8px; background: var(--surface-2); border: 1px solid var(--border); border-radius: var(--r-sm); font-size: 12px;"
                >
                  {{ c }}
                </div>
              }
            </div>
            <button
              class="fd-btn"
              type="button"
              (click)="onDownloadCodes()"
            >
              <faso-icon name="download" [size]="13" />
              {{ lang() === 'fr' ? 'Télécharger .txt' : 'Download .txt' }}
            </button>
          } @else {
            <button
              class="fd-btn danger"
              type="button"
              (click)="onRegenerateCodes()"
            >
              <faso-icon name="rotate" [size]="13" />
              {{
                lang() === 'fr'
                  ? 'Régénérer (invalide les anciens)'
                  : 'Regenerate (invalidate old)'
              }}
            </button>
          }
          <div class="fd-help" style="margin-top: 10px;">
            POST <span class="fd-mono">/api/admin/me/recovery-codes/regenerate</span>
          </div>
        </div>
      </div>

      <!-- Card Sessions actives -->
      <div class="fd-card">
        <div class="fd-card-h">
          <div class="fd-card-h-title">
            <faso-icon name="monitor" [size]="14" />
            {{ lang() === 'fr' ? 'Sessions actives' : 'Active sessions' }}
            <span style="color: var(--text-3); font-weight: 400;">
              · {{ sessions().length }}
            </span>
          </div>
        </div>
        <div
          class="fd-card-b"
          style="display: flex; flex-direction: column; gap: 8px;"
        >
          @for (s of displayedSessions(); track s.id) {
            <div
              style="display: flex; align-items: center; gap: 10px; padding: 8px 0; border-bottom: 1px solid var(--border);"
            >
              <div style="flex: 1; min-width: 0;">
                <div style="font-size: 12.5px; font-weight: 500;">
                  {{ s.device }}
                  @if (s.current) {
                    <span class="fd-chip ok" style="font-size: 10px; margin-left: 4px;">
                      {{ lang() === 'fr' ? 'courante' : 'current' }}
                    </span>
                  }
                </div>
                <div class="fd-mono" style="font-size: 11px; color: var(--text-3);">
                  {{ s.ip }} · {{ s.lastActive }}
                </div>
              </div>
            </div>
          }
          <a
            class="fd-btn ghost sm"
            routerLink="/admin/sessions"
            style="align-self: flex-start;"
          >
            {{
              lang() === 'fr' ? 'Voir toutes les sessions' : 'View all sessions'
            }}
            <faso-icon name="chevR" [size]="12" />
          </a>
        </div>
      </div>
    </div>
  `,
  styles: [
    `
      :host {
        display: contents;
      }
      .me-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 16px;
      }
      @media (max-width: 920px) {
        .me-grid {
          grid-template-columns: 1fr;
        }
      }
    `,
  ],
})
export class MeSecurityPage {
  readonly lang = input<AdminLang>('fr');

  private readonly fb = inject(FormBuilder);
  private readonly route = inject(ActivatedRoute);

  protected readonly passwordForm: FormGroup = this.fb.group(
    {
      currentPassword: ['', [Validators.required, Validators.minLength(8)]],
      newPassword: ['', [Validators.required, Validators.minLength(12)]],
      confirmPassword: ['', [Validators.required, Validators.minLength(12)]],
    },
    { validators: [this.passwordsMatchValidator] },
  );

  protected readonly passwordError = signal<string | null>(null);
  protected readonly passwordChanged = signal<boolean>(false);

  protected readonly passkeys = signal<readonly PasskeySummary[]>([
    {
      id: 'pk-1',
      label: 'YubiKey 5C — Bureau Ouagadougou',
      addedAt: '12 mars 2024',
    },
  ]);

  protected readonly totpEnrolled = signal<boolean>(false);
  protected readonly totpStepperOpen = signal<boolean>(false);
  protected readonly totpCode = signal<string>('');
  protected readonly totpSecret = 'JBSWY3DPEHPK3PXP6QFTAUDOGYZWJ4LE';

  protected readonly recoveryTotal = 10;
  protected readonly recoveryRemaining = signal<number>(8);
  protected readonly regeneratedCodes = signal<readonly string[]>([]);

  protected readonly sessions = signal<readonly SessionSummary[]>([
    {
      id: 's-1',
      device: 'Dell Latitude 7440 · Firefox 124',
      ip: '196.28.111.18',
      lastActive: '1 min',
      current: true,
    },
    {
      id: 's-2',
      device: 'iPhone 15 Pro · Safari Mobile',
      ip: '41.207.99.4',
      lastActive: '4 h',
      current: false,
    },
  ]);

  protected readonly displayedSessions = computed(() =>
    this.sessions().slice(0, 5),
  );

  private readonly queryParams = toSignal(this.route.queryParamMap, {
    initialValue: this.route.snapshot.queryParamMap,
  });

  protected readonly forceReenroll = computed(
    () => this.queryParams().get('force-reenroll') === 'true',
  );

  private passwordsMatchValidator(group: FormGroup): { mismatch: true } | null {
    const newPwd = group.get('newPassword')?.value;
    const confirm = group.get('confirmPassword')?.value;
    return newPwd && confirm && newPwd !== confirm ? { mismatch: true } : null;
  }

  protected onChangePassword(): void {
    this.passwordError.set(null);
    this.passwordChanged.set(false);
    if (this.passwordForm.invalid) {
      const mismatch = this.passwordForm.errors?.['mismatch'];
      this.passwordError.set(
        mismatch
          ? this.lang() === 'fr'
            ? 'Les mots de passe ne correspondent pas.'
            : 'Passwords do not match.'
          : this.lang() === 'fr'
            ? 'Mot de passe trop court (≥ 12 caractères).'
            : 'Password too short (≥ 12 characters).',
      );
      return;
    }
    // Hook future : POST /api/admin/me/password.
    this.passwordChanged.set(true);
    this.passwordForm.reset();
  }

  protected onEnrollPasskey(): void {
    // Hook future : @simplewebauthn/browser startRegistration() puis
    // POST /api/admin/me/passkeys/enroll/{begin,finish}.
    const id = `pk-${Date.now()}`;
    this.passkeys.update((arr) => [
      ...arr,
      {
        id,
        label: 'PassKey ' + (arr.length + 1),
        addedAt: this.lang() === 'fr' ? "à l'instant" : 'just now',
      },
    ]);
  }

  protected onRemovePasskey(id: string): void {
    this.passkeys.update((arr) => arr.filter((p) => p.id !== id));
  }

  protected onTotpVerify(value: string): void {
    void value;
  }

  protected onTotpFinish(): void {
    // Hook future : POST /api/admin/me/totp/enroll/finish.
    this.totpEnrolled.set(true);
    this.totpStepperOpen.set(false);
    this.totpCode.set('');
  }

  protected onDisableTotp(): void {
    this.totpEnrolled.set(false);
  }

  protected onRegenerateCodes(): void {
    // Hook future : POST /api/admin/me/recovery-codes/regenerate.
    const codes: string[] = [];
    for (let i = 0; i < this.recoveryTotal; i++) {
      codes.push(
        `${this.randomGroup(4)}-${this.randomGroup(4)}`,
      );
    }
    this.regeneratedCodes.set(codes);
    this.recoveryRemaining.set(this.recoveryTotal);
  }

  protected onDownloadCodes(): void {
    const codes = this.regeneratedCodes();
    if (codes.length === 0) return;
    const blob = new Blob([codes.join('\n')], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'faso-recovery-codes.txt';
    a.click();
    URL.revokeObjectURL(url);
  }

  private randomGroup(len: number): string {
    const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
    let s = '';
    for (let i = 0; i < len; i++) {
      s += chars[Math.floor(Math.random() * chars.length)];
    }
    return s;
  }
}
