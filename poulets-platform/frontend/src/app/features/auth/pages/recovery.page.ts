// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso
//
// Page publique de récupération de compte. 3 modes via query params :
//  - ?mode=initiate (défaut) : email → POST /api/admin/auth/recovery/initiate
//  - ?token=<jwt>            : OTP 8 chiffres → POST /api/admin/auth/recovery/complete
//  - ?adminToken=<8digits>   : email + token 8 chiffres → POST .../complete
//
// Cf. DELTA-REQUIREMENTS-2026-04-30 §5.

import { CommonModule } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  signal,
} from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import {
  FormBuilder,
  ReactiveFormsModule,
  Validators,
  type FormGroup,
} from '@angular/forms';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { FasoIconComponent } from '../../admin/components-v2/faso-icon.component';
import { FasoOtpInputComponent } from '../../admin/components-v2/faso-otp-input.component';

type RecoveryMode = 'initiate' | 'token' | 'adminToken';

@Component({
  selector: 'faso-recovery-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    ReactiveFormsModule,
    RouterLink,
    FasoIconComponent,
    FasoOtpInputComponent,
  ],
  template: `
    <div class="recovery-shell">
      <div class="recovery-card">
        <div style="margin-bottom: 18px;">
          <a routerLink="/auth/login" class="back-link">
            <faso-icon
              name="chevR"
              [size]="13"
              style="transform: rotate(180deg);"
            />
            Retour
          </a>
        </div>

        @switch (mode()) {
          @case ('initiate') {
            <h1 class="title">Récupérer l'accès au compte</h1>
            <p class="body">
              Saisissez votre adresse e-mail. Si elle est connue, vous recevrez
              un lien magique de récupération valide 30 minutes.
            </p>

            @if (initiateSent()) {
              <div class="banner ok">
                <faso-icon name="check" [size]="14" />
                <div>
                  E-mail envoyé. Consultez votre boîte de réception (et les
                  indésirables).
                </div>
              </div>
            } @else {
              <form
                [formGroup]="initiateForm"
                (ngSubmit)="onInitiate()"
                class="form"
              >
                <label>E-mail</label>
                <input
                  type="email"
                  class="input"
                  formControlName="email"
                  autocomplete="email"
                  required
                />
                <button
                  type="submit"
                  class="btn primary"
                  [disabled]="initiateForm.invalid || initiating()"
                >
                  @if (initiating()) {
                    Envoi…
                  } @else {
                    Envoyer le lien
                  }
                </button>
              </form>
            }

            <div class="meta">
              POST <span class="mono">/api/admin/auth/recovery/initiate</span>
            </div>
          }

          @case ('token') {
            <h1 class="title">Récupération via lien magique</h1>
            <p class="body">
              @if (recoverySessionId()) {
                Saisissez le code à 8 chiffres affiché ci-dessous sur ce
                même onglet. C'est la preuve que vous tenez l'e-mail
                <em>et</em> ce navigateur (channel-binding).
              } @else {
                Cliquez sur "Vérifier le lien" pour valider votre e-mail.
                Un code à 8 chiffres sera ensuite affiché à saisir.
              }
            </p>

            @if (completed()) {
              <div class="banner ok">
                <faso-icon name="check" [size]="14" />
                <div>
                  Accès restauré. Vous devez maintenant réenrôler un facteur MFA.
                </div>
              </div>
            } @else if (recoverySessionId()) {
              <div class="otp-display">
                <div class="otp-display-label">Code à saisir</div>
                <div class="otp-display-value">{{ recoveryOtpDisplay() }}</div>
              </div>
              <div class="otp-row">
                <faso-otp-input
                  [length]="8"
                  [(value)]="otp"
                  (complete)="onOtpComplete($event)"
                />
              </div>
              <button
                type="button"
                class="btn primary"
                [disabled]="otp().length !== 8 || verifying()"
                (click)="onCompleteToken()"
              >
                @if (verifying()) {
                  Vérification…
                } @else {
                  Vérifier
                }
              </button>
              @if (errorMsg(); as err) {
                <div class="banner err">
                  <faso-icon name="alertTri" [size]="14" />
                  <div>{{ err }}</div>
                </div>
              }
            } @else {
              <button
                type="button"
                class="btn primary"
                [disabled]="verifying()"
                (click)="onCompleteToken()"
              >
                @if (verifying()) {
                  Vérification…
                } @else {
                  Vérifier le lien
                }
              </button>
              @if (errorMsg(); as err) {
                <div class="banner err">
                  <faso-icon name="alertTri" [size]="14" />
                  <div>{{ err }}</div>
                </div>
              }
            }
            <div class="meta">
              POST <span class="mono">/api/admin/auth/recovery/verify-link</span>
              → <span class="mono">/verify-otp</span>
            </div>
          }

          @case ('adminToken') {
            <h1 class="title">Récupération initiée par un administrateur</h1>
            <p class="body">
              Saisissez votre e-mail et le code à 8 chiffres reçu.
            </p>

            @if (completed()) {
              <div class="banner ok">
                <faso-icon name="check" [size]="14" />
                <div>
                  Accès restauré. Vous devez maintenant réenrôler un facteur MFA.
                </div>
              </div>
            } @else {
              <form
                [formGroup]="adminTokenForm"
                (ngSubmit)="onCompleteAdminToken()"
                class="form"
              >
                <label>E-mail</label>
                <input
                  type="email"
                  class="input"
                  formControlName="email"
                  autocomplete="email"
                  required
                />
                <label>Code à 8 chiffres</label>
                <input
                  type="text"
                  inputmode="numeric"
                  class="input mono"
                  formControlName="token"
                  maxlength="8"
                  required
                />
                <button
                  type="submit"
                  class="btn primary"
                  [disabled]="adminTokenForm.invalid || verifying()"
                >
                  @if (verifying()) {
                    Vérification…
                  } @else {
                    Vérifier
                  }
                </button>
                @if (errorMsg(); as err) {
                  <div class="banner err">
                    <faso-icon name="alertTri" [size]="14" />
                    <div>{{ err }}</div>
                  </div>
                }
              </form>
            }
            <div class="meta">
              POST <span class="mono">/api/admin/auth/recovery/complete</span>
            </div>
          }
        }
      </div>
    </div>
  `,
  styles: [
    `
      :host {
        display: block;
        min-height: 100vh;
        background: #f5f6f8;
      }
      .recovery-shell {
        min-height: 100vh;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
      }
      .recovery-card {
        width: 100%;
        max-width: 480px;
        background: #fff;
        border-radius: 12px;
        padding: 32px;
        box-shadow: 0 6px 24px rgba(0, 0, 0, 0.06);
      }
      .back-link {
        font-size: 12px;
        color: #555;
        display: inline-flex;
        align-items: center;
        gap: 4px;
        text-decoration: none;
      }
      .back-link:hover {
        color: #111;
      }
      .title {
        font-size: 22px;
        font-weight: 700;
        margin: 0 0 8px;
        color: #111;
      }
      .body {
        font-size: 13px;
        color: #555;
        line-height: 1.5;
        margin: 0 0 18px;
      }
      .form {
        display: flex;
        flex-direction: column;
        gap: 8px;
      }
      .form label {
        font-size: 12px;
        color: #444;
        font-weight: 500;
      }
      .input {
        width: 100%;
        padding: 10px 12px;
        border: 1px solid #d8dadf;
        border-radius: 8px;
        font-size: 13px;
        font-family: inherit;
      }
      .input.mono {
        font-family:
          ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
        letter-spacing: 0.08em;
      }
      .input:focus {
        outline: 2px solid #1b5e20;
        outline-offset: -1px;
      }
      .btn {
        padding: 10px 14px;
        border-radius: 8px;
        font-size: 13px;
        font-weight: 600;
        cursor: pointer;
        border: 1px solid #d8dadf;
        background: #fff;
      }
      .btn.primary {
        background: #1b5e20;
        color: #fff;
        border-color: #1b5e20;
        margin-top: 6px;
      }
      .btn.primary:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }
      .otp-row {
        display: flex;
        justify-content: center;
        margin: 14px 0 18px;
      }
      .otp-display {
        background: #fbf3d8;
        border: 2px solid #1b5e20;
        border-radius: 12px;
        padding: 18px 20px;
        margin: 16px 0 18px;
        text-align: center;
      }
      .otp-display-label {
        font-size: 11px;
        color: #1b5e20;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.06em;
      }
      .otp-display-value {
        font-family:
          ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
        font-size: 32px;
        color: #1b5e20;
        font-weight: 700;
        margin-top: 6px;
        letter-spacing: 0.16em;
      }
      .banner {
        margin: 14px 0;
        padding: 10px 12px;
        border-radius: 8px;
        font-size: 12.5px;
        display: flex;
        align-items: flex-start;
        gap: 8px;
      }
      .banner.ok {
        background: #ecf6ee;
        color: #1b5e20;
        border: 1px solid #b9dfc1;
      }
      .banner.err {
        background: #fdecec;
        color: #b71c1c;
        border: 1px solid #f5b7b7;
      }
      .meta {
        margin-top: 18px;
        font-size: 11px;
        color: #777;
      }
      .mono {
        font-family:
          ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      }
    `,
  ],
})
export class RecoveryPage {
  private readonly fb = inject(FormBuilder);
  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);

  private readonly queryParams = toSignal(this.route.queryParamMap, {
    initialValue: this.route.snapshot.queryParamMap,
  });

  protected readonly mode = computed<RecoveryMode>(() => {
    const qp = this.queryParams();
    if (qp.has('adminToken')) return 'adminToken';
    if (qp.has('token')) return 'token';
    return 'initiate';
  });

  protected readonly initiateForm: FormGroup = this.fb.group({
    email: ['', [Validators.required, Validators.email]],
  });

  protected readonly adminTokenForm: FormGroup = this.fb.group({
    email: ['', [Validators.required, Validators.email]],
    token: [
      '',
      [
        Validators.required,
        Validators.minLength(8),
        Validators.maxLength(8),
        Validators.pattern(/^\d{8}$/),
      ],
    ],
  });

  protected readonly otp = signal<string>('');
  protected readonly initiating = signal<boolean>(false);
  protected readonly initiateSent = signal<boolean>(false);
  protected readonly verifying = signal<boolean>(false);
  protected readonly completed = signal<boolean>(false);
  protected readonly errorMsg = signal<string | null>(null);

  /** KAYA-backed verify-link session id, populated once magic-link is verified. */
  protected readonly recoverySessionId = signal<string | null>(null);
  /** 8-digit OTP displayed on the same browser tab post-verify-link. */
  protected readonly recoveryOtpDisplay = signal<string>('');

  protected onInitiate(): void {
    if (this.initiateForm.invalid) return;
    this.initiating.set(true);
    this.errorMsg.set(null);
    const email = this.initiateForm.value.email as string;
    void fetch('/api/admin/auth/recovery/initiate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email }),
    })
      .then((r) => {
        // Always treat as success — anti-enumeration. Errors are network-level.
        if (!r.ok && r.status !== 202 && r.status !== 200) {
          throw new Error(`HTTP ${r.status}`);
        }
        this.initiateSent.set(true);
      })
      .catch((e: unknown) => {
        this.errorMsg.set(
          e instanceof Error ? e.message : 'Erreur réseau, réessayez.',
        );
      })
      .finally(() => this.initiating.set(false));
  }

  protected onOtpComplete(value: string): void {
    this.otp.set(value);
  }

  protected onCompleteToken(): void {
    // Phase 4.b.4 — when ?token=<jwt> is present we follow the magic-link
    // channel-binding flow : verify-link -> display OTP -> verify-otp.
    const token = this.queryParams().get('token');
    if (!token) {
      this.errorMsg.set('Token manquant.');
      return;
    }
    const sid = this.recoverySessionId();
    if (sid) {
      // Step B — submit OTP entered on this tab.
      if (this.otp().length !== 8) return;
      this.verifying.set(true);
      this.errorMsg.set(null);
      void fetch('/api/admin/auth/recovery/verify-otp', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ sessionId: sid, otpEntry: this.otp() }),
      })
        .then(async (r) => {
          if (!r.ok) {
            const err = await r.json().catch(() => ({}));
            throw new Error(err?.error?.toString() ?? `HTTP ${r.status}`);
          }
          this.completed.set(true);
          this.redirectToReenroll();
        })
        .catch((e: unknown) => {
          this.errorMsg.set(
            e instanceof Error ? e.message : 'OTP invalide.',
          );
        })
        .finally(() => this.verifying.set(false));
      return;
    }
    // Step A — first request : exchange magic-link for OTP display.
    this.verifying.set(true);
    this.errorMsg.set(null);
    void fetch('/api/admin/auth/recovery/verify-link', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token }),
    })
      .then(async (r) => {
        if (!r.ok) {
          const err = await r.json().catch(() => ({}));
          throw new Error(err?.error?.toString() ?? `HTTP ${r.status}`);
        }
        return r.json() as Promise<{ sessionId: string; otpDisplay: string }>;
      })
      .then((data) => {
        this.recoverySessionId.set(data.sessionId);
        this.recoveryOtpDisplay.set(data.otpDisplay);
      })
      .catch((e: unknown) => {
        this.errorMsg.set(
          e instanceof Error ? e.message : 'Lien invalide ou expiré.',
        );
      })
      .finally(() => this.verifying.set(false));
  }

  protected onCompleteAdminToken(): void {
    if (this.adminTokenForm.invalid) return;
    this.verifying.set(true);
    this.errorMsg.set(null);
    const { email, token } = this.adminTokenForm.value as {
      email: string;
      token: string;
    };
    void fetch('/api/admin/auth/recovery/complete', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tokenOrCode: token, email }),
    })
      .then(async (r) => {
        if (!r.ok) {
          const err = await r.json().catch(() => ({}));
          throw new Error(err?.error?.toString() ?? `HTTP ${r.status}`);
        }
        this.completed.set(true);
        this.redirectToReenroll();
      })
      .catch((e: unknown) => {
        this.errorMsg.set(
          e instanceof Error ? e.message : 'Code invalide.',
        );
      })
      .finally(() => this.verifying.set(false));
  }

  private redirectToReenroll(): void {
    setTimeout(() => {
      void this.router.navigate(['/admin/me/security'], {
        queryParams: { 'force-reenroll': 'true' },
      });
    }, 1200);
  }
}
