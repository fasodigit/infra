// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { CommonModule } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  effect,
  inject,
  signal,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import {
  MatDialog,
  MatDialogModule,
  MatDialogRef,
  MAT_DIALOG_DATA,
} from '@angular/material/dialog';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { startAuthentication } from '@simplewebauthn/browser';
import { firstValueFrom } from 'rxjs';

import { FasoOtpInputComponent } from './faso-otp-input.component';
import {
  StepUpMethodWire,
  StepUpService,
} from '../services/step-up.service';

/**
 * Guard top-level (Phase 4.b.7) — surveille `StepUpService.currentPending` et,
 * dès qu'une demande arrive, ouvre le modal Material `StepUpDialogComponent`.
 *
 * Insertion : monter une fois dans `app.component.html` ou un layout admin.
 */
@Component({
  selector: 'faso-step-up-guard',
  standalone: true,
  imports: [CommonModule, MatDialogModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `<ng-container></ng-container>`,
})
export class FasoStepUpGuardComponent {
  private readonly stepUp = inject(StepUpService);
  private readonly dialog = inject(MatDialog);
  private currentRef: MatDialogRef<StepUpDialogComponent> | null = null;

  constructor() {
    effect(() => {
      const pending = this.stepUp.currentPending();
      if (pending && !this.currentRef) {
        const ref = this.dialog.open(StepUpDialogComponent, {
          data: pending,
          disableClose: true,
          width: '480px',
          autoFocus: true,
        });
        this.currentRef = ref;
        ref.afterClosed().subscribe(() => {
          this.currentRef = null;
        });
      }
    });
  }
}

// ---------------------------------------------------------------------------
// Dialog interne — sélection de méthode + saisie de la preuve
// ---------------------------------------------------------------------------

interface DialogData {
  readonly sessionId: string;
  readonly methods: readonly StepUpMethodWire[];
  readonly expiresAt: string;
  readonly retryUrl: string;
}

const METHOD_LABELS: Record<StepUpMethodWire, string> = {
  passkey: 'PassKey (recommandé)',
  'push-approval': 'Approbation sur un autre appareil',
  totp: "Code de l'application TOTP (6 chiffres)",
  otp: 'Code par e-mail (8 chiffres)',
};

const METHOD_ICONS: Record<StepUpMethodWire, string> = {
  passkey: 'fingerprint',
  'push-approval': 'phonelink_lock',
  totp: 'lock_clock',
  otp: 'mail_lock',
};

@Component({
  selector: 'faso-step-up-dialog',
  standalone: true,
  imports: [
    CommonModule,
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatIconModule,
    MatProgressSpinnerModule,
    FasoOtpInputComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Confirmation requise</h2>
    <mat-dialog-content class="fd-step-up">
      <p>
        Cette opération est sensible. Veuillez confirmer votre identité —
        une nouvelle vérification est exigée toutes les 5 minutes.
      </p>

      @if (!selectedMethod()) {
        <ul class="fd-step-up__methods">
          @for (m of data.methods; track m) {
            <li>
              <button
                mat-stroked-button
                color="primary"
                type="button"
                (click)="selectMethod(m)"
              >
                <mat-icon aria-hidden="true">{{ icon(m) }}</mat-icon>
                <span>{{ label(m) }}</span>
              </button>
            </li>
          }
        </ul>
      } @else if (busy()) {
        <div class="fd-step-up__busy">
          <mat-spinner diameter="32"></mat-spinner>
          <p>{{ statusText() }}</p>
        </div>
      } @else if (selectedMethod() === 'totp') {
        <p>Saisissez le code de votre application TOTP (6 chiffres).</p>
        <faso-otp-input [length]="6" [(value)]="totpCode" />
        <button
          mat-flat-button
          color="primary"
          type="button"
          [disabled]="totpCode().length !== 6"
          (click)="submitTotp()"
        >
          Valider
        </button>
      } @else if (selectedMethod() === 'otp') {
        <p>
          Un code à 8 chiffres a été envoyé à votre adresse e-mail.
          Saisissez-le ici.
        </p>
        <faso-otp-input [length]="8" [(value)]="otpCode" />
        <button
          mat-flat-button
          color="primary"
          type="button"
          [disabled]="otpCode().length !== 8 || !otpId()"
          (click)="submitOtp()"
        >
          Valider
        </button>
      }

      @if (errorMsg()) {
        <p class="fd-step-up__error" role="alert">{{ errorMsg() }}</p>
      }
    </mat-dialog-content>

    <mat-dialog-actions align="end">
      <button mat-button type="button" (click)="cancel()">Annuler</button>
    </mat-dialog-actions>
  `,
  styles: [
    `
      .fd-step-up__methods {
        list-style: none;
        margin: 0;
        padding: 0;
        display: grid;
        gap: 12px;
      }
      .fd-step-up__methods li button {
        width: 100%;
        justify-content: flex-start;
        gap: 8px;
      }
      .fd-step-up__busy {
        display: flex;
        align-items: center;
        gap: 12px;
      }
      .fd-step-up__error {
        color: var(--mat-sys-color-error, #b00020);
        font-size: 0.9rem;
      }
    `,
  ],
})
export class StepUpDialogComponent {
  readonly data: DialogData = inject<DialogData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<StepUpDialogComponent>);
  private readonly stepUp = inject(StepUpService);

  readonly selectedMethod = signal<StepUpMethodWire | null>(null);
  readonly busy = signal(false);
  readonly statusText = signal('Vérification en cours…');
  readonly errorMsg = signal<string | null>(null);

  readonly totpCode = signal('');
  readonly otpCode = signal('');
  readonly otpId = signal<string | null>(null);

  protected label(m: StepUpMethodWire): string {
    return METHOD_LABELS[m];
  }
  protected icon(m: StepUpMethodWire): string {
    return METHOD_ICONS[m];
  }

  selectMethod(m: StepUpMethodWire): void {
    this.errorMsg.set(null);
    this.selectedMethod.set(m);
    if (m === 'passkey') void this.runPasskey();
    else if (m === 'push-approval') void this.runPushApproval();
    else if (m === 'otp') void this.issueOtp();
    // TOTP — wait for user input then click Valider.
  }

  cancel(): void {
    this.stepUp.publishToken(this.data.sessionId, null);
    this.ref.close();
  }

  // ── PASSKEY ───────────────────────────────────────────────────────────

  private async runPasskey(): Promise<void> {
    this.busy.set(true);
    this.statusText.set('Approbation PassKey en cours…');
    try {
      // The auth-ms WebAuthnService exposes `authenticateBegin/Finish` —
      // here the modal triggers FIDO2 user-verification directly via
      // @simplewebauthn/browser using the publicKey options the BFF passes
      // through. For Phase 4.b.7 iter 1 we call the existing passkey-auth
      // begin endpoint and submit the assertion as proof.
      const optionsResp = await fetch('/api/admin/passkey/auth-begin', {
        method: 'POST',
        credentials: 'include',
      });
      if (!optionsResp.ok) throw new Error('passkey-begin failed');
      const options = (await optionsResp.json()) as Record<string, unknown>;
      const assertion = await startAuthentication({ optionsJSON: options as never });
      const proof = JSON.stringify(assertion);
      await this.submitVerify('passkey', proof);
    } catch (err) {
      this.fail('Échec PassKey : ' + (err as Error).message);
    }
  }

  // ── PUSH APPROVAL ─────────────────────────────────────────────────────

  private async runPushApproval(): Promise<void> {
    this.busy.set(true);
    this.statusText.set('En attente de votre approbation sur votre autre appareil…');
    try {
      // The companion device pushes the approval; we poll the status
      // endpoint and submit the approval requestId as proof when GRANTED.
      // For now the proof is the approval requestId provided by Stream
      // 4.b.5 — TODO Phase 4.b.5 wiring to fetch the requestId via the
      // /admin/auth/push-approval/initiate route.
      const ok = await this.stepUp.pollUntilVerified(this.data.sessionId, 5 * 60 * 1000, 3000);
      if (ok) {
        // The session is already VERIFIED server-side (filter satisfied);
        // we still need a stepUpToken — verify with proof = sessionId so
        // auth-ms can re-issue. Stream 4.b.5 may swap this for a real
        // approval requestId.
        await this.submitVerify('push-approval', this.data.sessionId);
      } else {
        this.fail('Approbation refusée ou expirée.');
      }
    } catch (err) {
      this.fail('Échec push-approval : ' + (err as Error).message);
    }
  }

  // ── TOTP ─────────────────────────────────────────────────────────────

  async submitTotp(): Promise<void> {
    if (this.busy()) return;
    this.busy.set(true);
    await this.submitVerify('totp', this.totpCode());
  }

  // ── OTP ─────────────────────────────────────────────────────────────

  private async issueOtp(): Promise<void> {
    this.busy.set(true);
    this.statusText.set('Envoi du code par e-mail…');
    try {
      const resp = await fetch('/api/admin/otp/issue', {
        method: 'POST',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ method: 'email', purpose: 'admin-login' }),
      });
      if (!resp.ok) throw new Error('otp-issue failed');
      const data = (await resp.json()) as { otpId?: string };
      if (!data.otpId) throw new Error('missing otpId');
      this.otpId.set(data.otpId);
      this.busy.set(false);
    } catch (err) {
      this.fail('Échec d’envoi : ' + (err as Error).message);
    }
  }

  async submitOtp(): Promise<void> {
    if (this.busy() || !this.otpId()) return;
    this.busy.set(true);
    const proof = `${this.otpId()}:${this.otpCode()}`;
    await this.submitVerify('otp', proof);
  }

  // ── Common verify ────────────────────────────────────────────────────

  private async submitVerify(method: StepUpMethodWire, proof: string): Promise<void> {
    this.statusText.set('Vérification…');
    try {
      const result = await firstValueFrom(
        this.stepUp.verify(this.data.sessionId, method, proof),
      );
      this.stepUp.publishToken(this.data.sessionId, result.stepUpToken);
      this.ref.close();
    } catch (err) {
      this.fail('Vérification refusée. Réessayez.');
      void err;
    }
  }

  private fail(msg: string): void {
    this.busy.set(false);
    this.errorMsg.set(msg);
    // Allow retrying another method.
    this.selectedMethod.set(null);
    this.totpCode.set('');
    this.otpCode.set('');
    this.otpId.set(null);
  }
}
