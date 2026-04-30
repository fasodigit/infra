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
import {
  FormBuilder,
  ReactiveFormsModule,
  Validators,
} from '@angular/forms';
import { toSignal } from '@angular/core/rxjs-interop';
import { TranslateModule } from '@ngx-translate/core';
import {
  FasoIconComponent,
  FasoOtpInputComponent,
} from '../components-v2';
import type { AdminLang } from '../models/admin.model';

@Component({
  selector: 'faso-break-glass-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    ReactiveFormsModule,
    TranslateModule,
    FasoIconComponent,
    FasoOtpInputComponent,
  ],
  template: `
    <div class="fd-page-head">
      <div>
        <div
          class="fd-h1"
          style="display: flex; align-items: center; gap: 10px;"
        >
          <faso-icon name="flame" [size]="22" style="color: var(--danger);" />
          Break-Glass
        </div>
        <div class="fd-page-sub">
          {{
            lang() === 'fr'
              ? 'Élévation temporaire ADMIN → SUPER-ADMIN pendant 4h. Tous les SUPER-ADMIN sont notifiés.'
              : 'Temporary ADMIN → SUPER-ADMIN elevation for 4h. All SUPER-ADMINs are notified.'
          }}
        </div>
      </div>
    </div>

    <div class="fd-banner danger">
      <faso-icon name="alertTri" [size]="18" />
      <div class="fd-banner-body">
        <strong>
          {{
            lang() === 'fr'
              ? 'Action sensible · audit immutable PostgreSQL WAL'
              : 'Sensitive action · immutable PostgreSQL WAL audit'
          }}
        </strong>
        <small>
          {{
            lang() === 'fr'
              ? 'Toute activation génère un événement Redpanda admin.break_glass.activated et notifie immédiatement les 2 SUPER-ADMIN. Auto-révocation à T+4h.'
              : 'Activation publishes admin.break_glass.activated and notifies all SUPER-ADMINs. Auto-revoke at T+4h.'
          }}
        </small>
      </div>
    </div>

    <form
      [formGroup]="form"
      style="display: grid; grid-template-columns: 1.4fr 1fr; gap: 16px;"
    >
      <div class="fd-card">
        <div class="fd-card-h">
          <div class="fd-card-h-title">
            {{
              lang() === 'fr'
                ? "Demande d'élévation"
                : 'Elevation request'
            }}
          </div>
        </div>
        <div class="fd-card-b">
          <div style="margin-bottom: 16px;">
            <div class="fd-help" style="margin-bottom: 6px;">
              {{
                lang() === 'fr' ? 'Capacité visée' : 'Target capability'
              }}
            </div>
            <select class="fd-select" formControlName="capability">
              <option value="db">
                {{
                  lang() === 'fr'
                    ? 'Accès direct base auth-ms (réplication, schéma)'
                    : 'Direct auth-ms DB access (replication, schema)'
                }}
              </option>
              <option value="grant">
                {{
                  lang() === 'fr'
                    ? 'Octroi rôle SUPER-ADMIN'
                    : 'Grant SUPER-ADMIN role'
                }}
              </option>
              <option value="settings">
                {{
                  lang() === 'fr'
                    ? 'Modifier paramètres critiques'
                    : 'Modify critical settings'
                }}
              </option>
            </select>
          </div>

          <div style="margin-bottom: 16px;">
            <div
              class="fd-help"
              style="margin-bottom: 6px; display: flex; justify-content: space-between;"
            >
              <span>
                {{
                  lang() === 'fr'
                    ? 'Justification · ≥ 80 caractères'
                    : 'Justification · ≥ 80 chars'
                }}
              </span>
              <span
                class="fd-mono"
                [style.color]="
                  tooShort() ? 'var(--danger)' : 'var(--ok)'
                "
              >
                {{ justifLen() }} / 80
              </span>
            </div>
            <textarea
              class="fd-textarea"
              rows="5"
              formControlName="justification"
            ></textarea>
          </div>

          <div style="margin-bottom: 16px;">
            <div class="fd-help" style="margin-bottom: 6px;">
              {{
                lang() === 'fr'
                  ? 'Confirmation OTP (8 chiffres)'
                  : 'OTP confirmation (8 digits)'
              }}
            </div>
            <faso-otp-input
              [length]="8"
              [(value)]="otp"
              (complete)="onOtpComplete($event)"
            />
            <div class="fd-help" style="margin-top: 6px;">
              <faso-icon name="clock" [size]="11" />
              {{
                lang() === 'fr' ? 'Code valide 4:58' : 'Code valid 4:58'
              }}
              ·
              <a class="fd-link">
                {{ lang() === 'fr' ? 'Renvoyer' : 'Resend' }}
              </a>
            </div>
          </div>

          <div class="fd-divider"></div>
          <div class="fd-row">
            <button type="button" class="fd-btn ghost">
              {{ lang() === 'fr' ? 'Annuler' : 'Cancel' }}
            </button>
            <div style="flex: 1;"></div>
            <button
              type="button"
              class="fd-btn lg danger"
              [disabled]="tooShort()"
              (click)="activate()"
            >
              <faso-icon name="flame" [size]="14" />
              {{
                lang() === 'fr'
                  ? 'Activer Break-Glass · 4h'
                  : 'Activate Break-Glass · 4h'
              }}
            </button>
          </div>
        </div>
      </div>

      <div style="display: flex; flex-direction: column; gap: 16px;">
        <div class="fd-card">
          <div class="fd-card-h">
            <div class="fd-card-h-title">
              {{
                lang() === 'fr'
                  ? 'Activations récentes'
                  : 'Recent activations'
              }}
            </div>
          </div>
          <div
            class="fd-card-b"
            style="display: flex; flex-direction: column; gap: 12px;"
          >
            <div
              style="padding: 10px; border-radius: var(--r-sm); background: var(--danger-soft); border: 1px solid rgba(198,40,40,0.18);"
            >
              <div style="display: flex; justify-content: space-between;">
                <span
                  style="font-size: 12.5px; font-weight: 600; color: var(--danger);"
                >
                  <faso-icon name="flame" [size]="11" /> Ibrahim Compaoré
                </span>
                <span
                  class="fd-mono"
                  style="font-size: 11px; color: var(--danger);"
                >
                  03:42:18 {{ lang() === 'fr' ? 'restant' : 'left' }}
                </span>
              </div>
              <div
                style="font-size: 11.5px; color: var(--text-2); margin-top: 4px;"
              >
                30 avril · 09:14 — Incident SEV-1 base état-civil
              </div>
            </div>
            <div style="padding: 10px;">
              <div style="font-size: 12.5px; font-weight: 500;">
                Fatoumata Kaboré
              </div>
              <div
                style="font-size: 11.5px; color: var(--text-3); margin-top: 2px;"
              >
                22 avril ·
                {{ lang() === 'fr' ? 'expirée' : 'expired' }} · audit
                7d12c4a8
              </div>
            </div>
            <div style="padding: 10px;">
              <div style="font-size: 12.5px; font-weight: 500;">
                Mariam Traoré
              </div>
              <div
                style="font-size: 11.5px; color: var(--text-3); margin-top: 2px;"
              >
                14 avril ·
                {{
                  lang() === 'fr'
                    ? 'révoquée manuellement'
                    : 'manually revoked'
                }}
              </div>
            </div>
          </div>
        </div>

        <div class="fd-card">
          <div class="fd-card-b">
            <div
              style="font-size: 11px; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600; margin-bottom: 8px;"
            >
              {{
                lang() === 'fr' ? 'Politique en vigueur' : 'Active policy'
              }}
            </div>
            <div
              style="display: grid; grid-template-columns: auto 1fr; gap: 6px 14px; font-size: 12.5px;"
            >
              <span style="color: var(--text-3);">TTL</span>
              <span class="fd-mono">14 400 s · 4h</span>
              <span style="color: var(--text-3);">
                {{ lang() === 'fr' ? 'Justif. min' : 'Min justif.' }}
              </span>
              <span class="fd-mono">80 chars</span>
              <span style="color: var(--text-3);">OTP</span>
              <span class="fd-mono">required</span>
              <span style="color: var(--text-3);">Notif</span>
              <span>
                {{
                  lang() === 'fr'
                    ? 'Tous SUPER-ADMIN'
                    : 'All SUPER-ADMINs'
                }}
              </span>
              <span style="color: var(--text-3);">Audit</span>
              <span>immutable WAL</span>
            </div>
          </div>
        </div>
      </div>
    </form>
  `,
  styles: [`:host { display: contents; }`],
})
export class BreakGlassPage {
  readonly lang = input<AdminLang>('fr');

  private readonly fb = inject(FormBuilder);

  protected readonly form = this.fb.nonNullable.group({
    capability: this.fb.nonNullable.control<'db' | 'grant' | 'settings'>('db'),
    justification: this.fb.nonNullable.control<string>(
      'Incident SEV-1 base de données état-civil — perte de réplica primaire à Bobo, escalade nécessaire pour exécuter pg_basebackup et restaurer la réplication. Coordination avec équipe DBA en cours.',
      [Validators.minLength(80)],
    ),
  });

  protected readonly otp = signal<string>('');

  // Watch justification length reactively for the counter.
  private readonly justifValue = toSignal(
    this.form.controls.justification.valueChanges,
    { initialValue: this.form.controls.justification.value },
  );

  protected readonly justifLen = computed(
    () => (this.justifValue() ?? '').length,
  );
  protected readonly tooShort = computed(() => this.justifLen() < 80);

  protected onOtpComplete(value: string): void {
    void value;
    // Hook future : AdminOtpService.verifyOtp + AdminBreakGlassService.activate.
  }

  protected activate(): void {
    if (this.tooShort()) return;
    // Hook future : POST /api/admin/break-glass.
  }
}
