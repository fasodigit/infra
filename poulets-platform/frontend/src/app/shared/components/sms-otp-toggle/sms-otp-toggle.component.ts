// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import {
  ChangeDetectionStrategy,
  Component,
  inject,
  signal,
  output,
} from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatSlideToggleModule, MatSlideToggleChange } from '@angular/material/slide-toggle';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';

import { SmsOtpService } from './sms-otp.service';

/**
 * SMS OTP fallback toggle.
 *
 * Offers users the choice to receive their one-time password by SMS
 * instead of e-mail. Embeds on signup / verification pages.
 *
 * Emits `channelChange` so the host form can react.
 */
@Component({
  selector: 'app-sms-otp-toggle',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    FormsModule,
    MatSlideToggleModule,
    MatFormFieldModule,
    MatInputModule,
    MatButtonModule,
    MatIconModule,
  ],
  template: `
    <section class="sms-otp" data-testid="sms-otp-toggle">
      <div class="row">
        <mat-slide-toggle
          [checked]="smsEnabled()"
          (change)="onToggle($event)"
          data-testid="sms-toggle"
        >
          Recevoir OTP par SMS
        </mat-slide-toggle>
        <small class="hint">
          Utile si vous n'avez pas accès à votre e-mail depuis votre téléphone.
        </small>
      </div>

      @if (smsEnabled()) {
        <div class="sms-fields">
          <mat-form-field appearance="outline" class="full">
            <mat-label>Numéro de téléphone (+226)</mat-label>
            <input
              matInput
              type="tel"
              [(ngModel)]="phone"
              name="smsPhone"
              inputmode="numeric"
              placeholder="+22670123456"
              maxlength="16"
              data-testid="sms-phone-input"
            />
          </mat-form-field>
          <button
            mat-stroked-button
            color="primary"
            type="button"
            (click)="requestOtp()"
            [disabled]="!phone().length || requesting()"
            data-testid="sms-request-btn"
          >
            <mat-icon>sms</mat-icon>
            @if (requesting()) {
              Envoi…
            } @else {
              Envoyer le code SMS
            }
          </button>
          @if (lastResponse(); as r) {
            <p class="result" [class.ok]="r.sent" data-testid="sms-result">
              @if (r.sent) {
                Code envoyé — vérifiez vos SMS.
              } @else {
                {{ r.message || 'Erreur lors de l\\'envoi du SMS.' }}
              }
            </p>
          }
        </div>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    .sms-otp {
      padding: 12px;
      border: 1px dashed #c8c8c8;
      border-radius: 8px;
      background: #fafafa;
    }
    .row {
      display: flex;
      flex-direction: column;
      gap: 4px;
    }
    .hint {
      color: #666;
      font-size: 0.85rem;
    }
    .sms-fields {
      margin-top: 12px;
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .full { width: 100%; }
    .result {
      margin: 0;
      font-size: 0.9rem;
      color: #c62828;
    }
    .result.ok { color: #2e7d32; }
  `],
})
export class SmsOtpToggleComponent {
  private readonly svc = inject(SmsOtpService);

  /** Fired when user toggles between SMS / email OTP channels. */
  readonly channelChange = output<'sms' | 'email'>();

  readonly smsEnabled = signal(this.svc.channel() === 'sms');
  readonly phone = signal('');
  readonly requesting = signal(false);
  readonly lastResponse = signal<{ sent: boolean; message?: string } | null>(null);

  onToggle(event: MatSlideToggleChange): void {
    const channel: 'sms' | 'email' = event.checked ? 'sms' : 'email';
    this.smsEnabled.set(event.checked);
    this.svc.setChannel(channel);
    this.channelChange.emit(channel);
  }

  requestOtp(): void {
    if (this.requesting()) return;
    this.requesting.set(true);

    this.svc.send({ phone: this.phone() }).subscribe({
      next: (res) => {
        this.lastResponse.set({ sent: res.sent, message: res.message });
        this.requesting.set(false);
      },
      error: () => {
        this.lastResponse.set({ sent: false, message: 'Erreur réseau' });
        this.requesting.set(false);
      },
    });
  }
}
