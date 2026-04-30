// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { CommonModule } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  input,
  signal,
} from '@angular/core';
import { TranslateModule } from '@ngx-translate/core';
import {
  FasoIconComponent,
  FasoOtpInputComponent,
} from '../components-v2';
import type { AdminLang } from '../models/admin.model';

type MfaTab = 'passkey' | 'totp' | 'backup';

interface BackupCode {
  readonly idx: number;
  readonly code: string;
  readonly used: boolean;
}

@Component({
  selector: 'faso-mfa-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    TranslateModule,
    FasoIconComponent,
    FasoOtpInputComponent,
  ],
  template: `
    <div class="fd-page-head">
      <div>
        <div class="fd-h1">
          {{ lang() === 'fr' ? 'MFA · Enrôlement' : 'MFA · Enrollment' }}
        </div>
        <div class="fd-page-sub">
          {{
            lang() === 'fr'
              ? 'PassKey, TOTP et codes de récupération. Politique applicable : SUPER-ADMIN + ADMIN obligatoires.'
              : 'PassKey, TOTP and recovery codes. Policy: SUPER-ADMIN + ADMIN required.'
          }}
        </div>
      </div>
    </div>

    <div class="fd-card" style="margin-bottom: 16px;">
      <div style="display: flex; border-bottom: 1px solid var(--border);">
        @for (t of tabs; track t.id) {
          <button
            type="button"
            (click)="tab.set(t.id)"
            [style.padding]="'14px 20px'"
            [style.background]="'none'"
            [style.border]="'none'"
            [style.borderBottom]="
              tab() === t.id
                ? '2px solid var(--primary)'
                : '2px solid transparent'
            "
            [style.color]="
              tab() === t.id ? 'var(--primary)' : 'var(--text-2)'
            "
            [style.fontWeight]="tab() === t.id ? 600 : 500"
            [style.fontSize]="'13px'"
            [style.cursor]="'pointer'"
            [style.display]="'flex'"
            [style.alignItems]="'center'"
            [style.gap]="'8px'"
          >
            <faso-icon [name]="t.icon" [size]="14" />
            {{ lang() === 'fr' ? t.labelFr : t.labelEn }}
            <span
              class="fd-chip muted"
              style="font-size: 10.5px; padding: 0 6px;"
            >
              {{ t.count }}
            </span>
          </button>
        }
      </div>

      @switch (tab()) {
        @case ('passkey') {
          <div class="fd-card-b">
            <div style="display: flex; gap: 16px;">
              <div style="flex: 1;">
                <div style="font-weight: 600; margin-bottom: 6px;">
                  {{
                    lang() === 'fr' ? 'Clés enregistrées' : 'Registered keys'
                  }}
                </div>
                <div
                  style="color: var(--text-3); font-size: 12.5px; margin-bottom: 14px;"
                >
                  {{
                    lang() === 'fr'
                      ? 'WebAuthn FIDO2 — résiste au phishing, hardware-backed.'
                      : 'WebAuthn FIDO2 — phishing-resistant, hardware-backed.'
                  }}
                </div>
                <div
                  style="padding: 14px 16px; border-radius: var(--r-md); border: 1px solid var(--border); background: var(--surface-2); display: flex; align-items: center; gap: 14px;"
                >
                  <div
                    style="width: 44px; height: 44px; border-radius: 10px; background: var(--primary); color: #fff; display: flex; align-items: center; justify-content: center;"
                  >
                    <faso-icon name="key" [size]="20" />
                  </div>
                  <div style="flex: 1;">
                    <div style="font-weight: 500;">
                      YubiKey 5C — Bureau Ouagadougou
                    </div>
                    <div style="font-size: 12px; color: var(--text-3);">
                      {{
                        lang() === 'fr'
                          ? 'Ajoutée 12 mars 2024 · utilisée à l’instant'
                          : 'Added Mar 12, 2024 · used just now'
                      }}
                    </div>
                  </div>
                  <button class="fd-btn ghost sm">
                    {{ lang() === 'fr' ? 'Renommer' : 'Rename' }}
                  </button>
                  <button
                    class="fd-btn ghost sm"
                    style="color: var(--danger);"
                  >
                    <faso-icon name="trash" [size]="12" />
                  </button>
                </div>
              </div>
              <div style="width: 280px;">
                <button
                  class="fd-btn primary lg"
                  style="width: 100%; justify-content: center;"
                >
                  <faso-icon name="plus" [size]="14" />
                  {{
                    lang() === 'fr'
                      ? 'Ajouter une PassKey'
                      : 'Add a PassKey'
                  }}
                </button>
                <div class="fd-help" style="margin-top: 8px; line-height: 1.5;">
                  {{
                    lang() === 'fr'
                      ? 'Active la fenêtre WebAuthn navigateur · '
                      : 'Triggers browser WebAuthn dialog · '
                  }}
                  <span class="fd-mono">&#64;simplewebauthn/browser</span>
                </div>
              </div>
            </div>
          </div>
        }

        @case ('totp') {
          <div class="fd-card-b">
            <div
              style="display: grid; grid-template-columns: 180px 1fr; gap: 22px; align-items: flex-start;"
            >
              <div
                style="width: 180px; height: 180px; padding: 12px; background: #fff; border: 1px solid var(--border); border-radius: var(--r-md);"
              >
                <svg viewBox="0 0 80 80" style="width: 100%; height: 100%;">
                  @for (cell of qrCells; track $index) {
                    <rect
                      [attr.x]="cell.x"
                      [attr.y]="cell.y"
                      width="3"
                      height="3"
                      [attr.fill]="cell.fill"
                    />
                  }
                  @for (corner of qrCorners; track $index) {
                    <g>
                      <rect
                        [attr.x]="corner[0]"
                        [attr.y]="corner[1]"
                        width="22"
                        height="22"
                        fill="#fff"
                      />
                      <rect
                        [attr.x]="corner[0] + 2"
                        [attr.y]="corner[1] + 2"
                        width="18"
                        height="18"
                        fill="#000"
                      />
                      <rect
                        [attr.x]="corner[0] + 5"
                        [attr.y]="corner[1] + 5"
                        width="12"
                        height="12"
                        fill="#fff"
                      />
                      <rect
                        [attr.x]="corner[0] + 8"
                        [attr.y]="corner[1] + 8"
                        width="6"
                        height="6"
                        fill="#000"
                      />
                    </g>
                  }
                </svg>
              </div>
              <div>
                <div style="font-weight: 600; margin-bottom: 8px;">
                  {{
                    lang() === 'fr'
                      ? 'Étape 1 · Scanner avec une app TOTP'
                      : 'Step 1 · Scan with a TOTP app'
                  }}
                </div>
                <div class="fd-help" style="margin-bottom: 14px;">
                  {{
                    lang() === 'fr'
                      ? 'Authy, Aegis, Google Authenticator, 1Password. Émetteur :'
                      : 'Authy, Aegis, Google Authenticator, 1Password. Issuer:'
                  }}
                  <span class="fd-mono">FasoDigitalisation</span>
                </div>
                <div
                  style="padding: 10px; background: var(--surface-2); border: 1px solid var(--border); border-radius: var(--r-sm); font-size: 12px; margin-bottom: 16px;"
                >
                  <div
                    style="font-size: 11px; color: var(--text-3); margin-bottom: 4px;"
                  >
                    {{
                      lang() === 'fr'
                        ? 'Ou saisir manuellement (base32) :'
                        : 'Or enter manually (base32):'
                    }}
                  </div>
                  <div class="fd-mono" style="word-break: break-all;">
                    {{ totpSecret }}
                  </div>
                </div>
                <div style="font-weight: 600; margin-bottom: 8px;">
                  {{
                    lang() === 'fr'
                      ? 'Étape 2 · Confirmer le code à 6 chiffres'
                      : 'Step 2 · Confirm 6-digit code'
                  }}
                </div>
                <faso-otp-input
                  [length]="6"
                  [(value)]="totpCode"
                  (complete)="onTotpComplete($event)"
                />
                <div style="margin-top: 14px; display: flex; gap: 8px;">
                  <button
                    class="fd-btn primary"
                    [disabled]="totpCode().length !== 6"
                  >
                    {{ lang() === 'fr' ? 'Activer TOTP' : 'Enable TOTP' }}
                  </button>
                  <button class="fd-btn ghost">
                    {{ lang() === 'fr' ? 'Annuler' : 'Cancel' }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        }

        @case ('backup') {
          <div class="fd-card-b">
            <div class="fd-banner warn">
              <faso-icon name="alertTri" [size]="16" />
              <div class="fd-banner-body">
                {{
                  lang() === 'fr'
                    ? 'Conservez ces codes en lieu sûr. Chacun est utilisable une seule fois. Ne les partagez jamais.'
                    : 'Keep these codes safe. Each is single-use. Never share them.'
                }}
              </div>
            </div>
            <div
              style="display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px; margin-top: 12px;"
            >
              @for (c of backupCodes; track c.idx) {
                <div
                  class="fd-mono"
                  [style.padding]="'10px 12px'"
                  [style.borderRadius]="'var(--r-sm)'"
                  [style.background]="
                    c.used ? 'var(--surface-3)' : 'var(--surface-2)'
                  "
                  [style.border]="'1px solid var(--border)'"
                  [style.fontSize]="'14px'"
                  [style.letterSpacing]="'0.04em'"
                  [style.color]="c.used ? 'var(--text-3)' : 'var(--text)'"
                  [style.textDecoration]="c.used ? 'line-through' : 'none'"
                >
                  <span style="color: var(--text-3); margin-right: 8px;">
                    {{ pad2(c.idx) }}
                  </span>
                  {{ c.code }}
                </div>
              }
            </div>
            <div style="display: flex; gap: 8px; margin-top: 16px;">
              <button class="fd-btn">
                <faso-icon name="download" [size]="13" />
                {{
                  lang() === 'fr' ? 'Télécharger .txt' : 'Download .txt'
                }}
              </button>
              <button class="fd-btn">
                {{ lang() === 'fr' ? 'Copier tout' : 'Copy all' }}
              </button>
              <button class="fd-btn ghost">
                {{ lang() === 'fr' ? 'Imprimer' : 'Print' }}
              </button>
              <div style="flex: 1;"></div>
              <button class="fd-btn danger">
                <faso-icon name="rotate" [size]="13" />
                {{
                  lang() === 'fr'
                    ? 'Régénérer (invalide les anciens)'
                    : 'Regenerate (invalidate old)'
                }}
              </button>
            </div>
          </div>
        }
      }
    </div>
  `,
  styles: [`:host { display: contents; }`],
})
export class MfaPage {
  readonly lang = input<AdminLang>('fr');

  protected readonly tab = signal<MfaTab>('passkey');
  protected readonly totpCode = signal<string>('');
  protected readonly totpSecret = 'JBSWY3DPEHPK3PXP6QFTAUDOGYZWJ4LE';

  protected readonly tabs = [
    { id: 'passkey' as const, labelFr: 'PassKey', labelEn: 'PassKey', icon: 'key' as const, count: 1 },
    { id: 'totp' as const, labelFr: 'TOTP', labelEn: 'TOTP', icon: 'qr' as const, count: 1 },
    { id: 'backup' as const, labelFr: 'Codes de récupération', labelEn: 'Backup codes', icon: 'shield' as const, count: 8 },
  ];

  protected readonly backupCodes: readonly BackupCode[] = [
    '7K2M-9XQF', '3PNH-VR8L', 'BD4Y-W6GE', 'QC8U-T2MN', '5JFR-PK91',
    'XHBV-43DM', 'LN7Y-EWA8', '9TQS-VC2P', 'M2KB-J6XR', 'RP4N-DYUH',
  ].map((code, i) => ({ idx: i + 1, code, used: i < 2 }));

  protected readonly qrCells: readonly { x: number; y: number; fill: string }[] = (() => {
    const cells: { x: number; y: number; fill: string }[] = [];
    for (let i = 0; i < 80; i++) {
      const x = (i * 7) % 80;
      const y = Math.floor((i * 13) / 80) * 4;
      const fill = (i * 7) % 5 < 3 ? '#000' : 'transparent';
      cells.push({ x, y, fill });
    }
    return cells;
  })();

  protected readonly qrCorners: readonly [number, number][] = [
    [0, 0],
    [58, 0],
    [0, 58],
  ];

  protected pad2(n: number): string {
    return n.toString().padStart(2, '0');
  }

  protected onTotpComplete(value: string): void {
    void value;
    // Hook future: AdminOtpService.verifyOtp / enable TOTP.
  }
}
