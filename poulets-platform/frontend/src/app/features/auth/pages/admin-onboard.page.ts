// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso
//
// Phase 4.b.4 — Magic-link channel-binding au signup ADMIN.
// Page publique 3-steps :
//   1. Vérification du lien magique (POST /api/admin/auth/onboard/verify-link)
//      → backend renvoie { sessionId, otpDisplay (8 chiffres) }
//   2. Saisie du même OTP sur ce même onglet (channel-binding) → POST
//      /api/admin/auth/onboard/verify-otp → backend valide + force MFA enrol.
//   3. Redirect vers le flow Kratos settings pour enrôlement
//      PassKey + TOTP + recovery codes.

import { CommonModule } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  signal,
} from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import { FasoIconComponent } from '../../admin/components-v2/faso-icon.component';
import { FasoOtpInputComponent } from '../../admin/components-v2/faso-otp-input.component';

type Step = 'verifying-link' | 'enter-otp' | 'redirecting' | 'error';

interface VerifyLinkResponse {
  sessionId: string;
  otpDisplay: string;
  expiresAt: string;
  email?: string;
}

interface VerifyOtpResponse {
  redirectPath?: string;
  kratosSettingsFlowId?: string | null;
  email?: string;
}

@Component({
  selector: 'faso-admin-onboard-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    TranslateModule,
    FasoIconComponent,
    FasoOtpInputComponent,
  ],
  template: `
    <div class="onboard-shell">
      <div class="onboard-card">
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

        <h1 class="title">{{ 'admin.onboard.title' | translate }}</h1>
        <p class="body">{{ 'admin.onboard.subtitle' | translate }}</p>

        @switch (step()) {
          @case ('verifying-link') {
            <div class="banner pending">
              <span class="spinner"></span>
              <div>
                {{ 'admin.onboard.verifyingLink' | translate }}
              </div>
            </div>
          }

          @case ('enter-otp') {
            <div class="otp-display-wrap">
              <div class="otp-display-label">
                {{ 'admin.onboard.otpDisplayed' | translate }}
              </div>
              <div class="otp-display-value" data-testid="otp-display">
                {{ otpDisplay() }}
              </div>
              <div class="otp-display-hint">
                {{ 'admin.onboard.expiresIn' | translate }}
                <strong>{{ countdown() }}</strong>
              </div>
            </div>

            <div class="entry-block">
              <label>{{ 'admin.onboard.enterOtp' | translate }}</label>
              <faso-otp-input
                [length]="8"
                [(value)]="otpEntry"
                (complete)="onOtpComplete($event)"
              />
              <button
                type="button"
                class="btn primary"
                [disabled]="otpEntry().length !== 8 || verifying()"
                (click)="onSubmitOtp()"
              >
                @if (verifying()) {
                  Vérification…
                } @else {
                  Valider
                }
              </button>
              @if (errorMsg(); as err) {
                <div class="banner err">
                  <faso-icon name="alertTri" [size]="14" />
                  <div>{{ err }}</div>
                </div>
              }
            </div>
          }

          @case ('redirecting') {
            <div class="banner ok">
              <faso-icon name="check" [size]="14" />
              <div>
                {{ 'admin.onboard.forceMfa' | translate }}
              </div>
            </div>
          }

          @case ('error') {
            <div class="banner err">
              <faso-icon name="alertTri" [size]="14" />
              <div>{{ errorMsg() }}</div>
            </div>
            <a routerLink="/auth/login" class="btn primary" style="margin-top: 14px;">
              Retour à la connexion
            </a>
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
      .onboard-shell {
        min-height: 100vh;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
      }
      .onboard-card {
        width: 100%;
        max-width: 520px;
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
      .otp-display-wrap {
        background: #fbf3d8;
        border: 2px solid #1b5e20;
        border-radius: 12px;
        padding: 22px 24px;
        margin: 18px 0 22px;
        text-align: center;
      }
      .otp-display-label {
        font-size: 12px;
        color: #1b5e20;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.06em;
      }
      .otp-display-value {
        font-family:
          ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
        font-size: 38px;
        color: #1b5e20;
        font-weight: 700;
        margin-top: 8px;
        letter-spacing: 0.18em;
      }
      .otp-display-hint {
        font-size: 12px;
        color: #5b6f5f;
        margin-top: 10px;
      }
      .entry-block {
        display: flex;
        flex-direction: column;
        gap: 10px;
        margin-top: 6px;
      }
      .entry-block label {
        font-size: 12px;
        color: #444;
        font-weight: 500;
      }
      .btn {
        padding: 10px 14px;
        border-radius: 8px;
        font-size: 13px;
        font-weight: 600;
        cursor: pointer;
        border: 1px solid #d8dadf;
        background: #fff;
        text-decoration: none;
        text-align: center;
        display: inline-block;
      }
      .btn.primary {
        background: #1b5e20;
        color: #fff;
        border-color: #1b5e20;
      }
      .btn.primary:disabled {
        opacity: 0.5;
        cursor: not-allowed;
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
      .banner.pending {
        background: #f3f6f4;
        color: #1b5e20;
        border: 1px solid #d8d8d8;
        align-items: center;
      }
      .spinner {
        width: 16px;
        height: 16px;
        border: 2px solid #1b5e20;
        border-top-color: transparent;
        border-radius: 50%;
        animation: faso-spin 0.7s linear infinite;
      }
      @keyframes faso-spin {
        to {
          transform: rotate(360deg);
        }
      }
    `,
  ],
})
export class AdminOnboardPage {
  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);

  private readonly queryParams = toSignal(this.route.queryParamMap, {
    initialValue: this.route.snapshot.queryParamMap,
  });

  protected readonly step = signal<Step>('verifying-link');
  protected readonly sessionId = signal<string | null>(null);
  protected readonly otpDisplay = signal<string>('');
  protected readonly expiresAt = signal<number>(0);
  protected readonly otpEntry = signal<string>('');
  protected readonly verifying = signal<boolean>(false);
  protected readonly errorMsg = signal<string | null>(null);

  protected readonly countdown = computed(() => {
    const exp = this.expiresAt();
    if (!exp) return '';
    const remaining = Math.max(0, Math.floor((exp - Date.now()) / 1000));
    const mm = Math.floor(remaining / 60).toString().padStart(2, '0');
    const ss = (remaining % 60).toString().padStart(2, '0');
    return `${mm}:${ss}`;
  });

  constructor() {
    const token = this.route.snapshot.queryParamMap.get('token');
    if (!token) {
      this.errorMsg.set('Lien invalide.');
      this.step.set('error');
      return;
    }
    void this.verifyLink(token);
    setInterval(() => {
      if (this.expiresAt() && Date.now() >= this.expiresAt()) {
        this.errorMsg.set('Le code a expiré. Demandez une nouvelle invitation.');
        this.step.set('error');
      }
    }, 1000);
  }

  protected onOtpComplete(value: string): void {
    this.otpEntry.set(value);
  }

  protected onSubmitOtp(): void {
    const sid = this.sessionId();
    const code = this.otpEntry();
    if (!sid || code.length !== 8 || this.verifying()) return;
    this.verifying.set(true);
    this.errorMsg.set(null);
    void fetch('/api/admin/auth/onboard/verify-otp', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sessionId: sid, otpEntry: code }),
    })
      .then(async (r) => {
        if (!r.ok) {
          const err = await r.json().catch(() => ({}));
          throw new Error(err?.error?.toString() ?? `HTTP ${r.status}`);
        }
        return r.json() as Promise<VerifyOtpResponse>;
      })
      .then((data) => {
        this.step.set('redirecting');
        const target = data.redirectPath ?? '/admin/me/security?force-mfa-enroll=true';
        const qp: Record<string, string> = { 'force-mfa-enroll': 'true' };
        if (data.kratosSettingsFlowId) qp['kratosFlow'] = data.kratosSettingsFlowId;
        setTimeout(() => {
          // Use absolute path so router does not prepend the current url tree.
          if (target.startsWith('/')) {
            void this.router.navigateByUrl(target);
          } else {
            void this.router.navigate(['/admin/me/security'], { queryParams: qp });
          }
        }, 800);
      })
      .catch((e: unknown) => {
        this.errorMsg.set(
          e instanceof Error ? e.message : 'OTP invalide ou session expirée.',
        );
      })
      .finally(() => this.verifying.set(false));
  }

  private async verifyLink(token: string): Promise<void> {
    try {
      const resp = await fetch('/api/admin/auth/onboard/verify-link', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token }),
      });
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({}));
        throw new Error(err?.error?.toString() ?? `HTTP ${resp.status}`);
      }
      const data = (await resp.json()) as VerifyLinkResponse;
      this.sessionId.set(data.sessionId);
      this.otpDisplay.set(data.otpDisplay);
      this.expiresAt.set(Date.parse(data.expiresAt));
      this.step.set('enter-otp');
    } catch (e) {
      this.errorMsg.set(
        e instanceof Error ? e.message : 'Lien invalide ou expiré.',
      );
      this.step.set('error');
    }
  }
}
