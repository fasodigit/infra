// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import {
  ChangeDetectionStrategy,
  Component,
  inject,
  signal,
  computed,
  input,
} from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute } from '@angular/router';
import {
  ReactiveFormsModule,
  FormBuilder,
  Validators,
  FormControl,
} from '@angular/forms';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';

import {
  MobileMoneyService,
  MobileMoneyProvider,
  MobileMoneyInitiateResponse,
} from './mobile-money.service';

interface ProviderOption {
  code: MobileMoneyProvider;
  label: string;
  emoji: string;
}

const PROVIDERS: ProviderOption[] = [
  { code: 'orange_money', label: 'Orange Money', emoji: '🟠' },
  { code: 'moov_africa', label: 'Moov Africa', emoji: '🔵' },
  { code: 'wave', label: 'Wave', emoji: '🌊' },
];

/**
 * Mobile Money payment form for the 3 Burkina Faso providers.
 *
 * Features:
 * - mat-select for provider (Orange Money / Moov Africa / Wave)
 * - phone input auto-prefixed with `+226`
 * - amount in FCFA
 * - Initiates payment via `MobileMoneyService` → BFF route
 *
 * Route: `/checkout/pay/:txId`
 */
@Component({
  selector: 'app-mobile-money-form',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    ReactiveFormsModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    MatButtonModule,
    MatIconModule,
    MatProgressSpinnerModule,
  ],
  template: `
    <section class="momo">
      <header class="head">
        <h1>Paiement Mobile Money</h1>
        <p>Référence&nbsp;: <strong>{{ referenceId() }}</strong></p>
      </header>

      <form [formGroup]="form" class="form" (ngSubmit)="onSubmit()" novalidate>
        <mat-form-field appearance="outline" class="full">
          <mat-label>Fournisseur</mat-label>
          <mat-select formControlName="provider" required>
            @for (p of providers; track p.code) {
              <mat-option [value]="p.code">
                <span class="emoji" aria-hidden="true">{{ p.emoji }}</span>
                {{ p.label }}
              </mat-option>
            }
          </mat-select>
        </mat-form-field>

        <mat-form-field appearance="outline" class="full">
          <mat-label>Téléphone</mat-label>
          <span matTextPrefix>+226&nbsp;</span>
          <input
            matInput
            type="tel"
            formControlName="phone"
            inputmode="numeric"
            placeholder="70123456"
            maxlength="10"
            required
          />
          @if (form.controls.phone.touched && form.controls.phone.invalid) {
            <mat-error>Numéro téléphone invalide (8 chiffres)</mat-error>
          }
        </mat-form-field>

        <mat-form-field appearance="outline" class="full">
          <mat-label>Montant (FCFA)</mat-label>
          <input
            matInput
            type="number"
            formControlName="amount"
            inputmode="numeric"
            min="100"
            step="100"
            required
          />
          @if (form.controls.amount.touched && form.controls.amount.invalid) {
            <mat-error>Montant minimum&nbsp;: 100 FCFA</mat-error>
          }
        </mat-form-field>

        <button
          mat-flat-button
          color="primary"
          type="submit"
          class="cta"
          [disabled]="form.invalid || submitting()"
        >
          @if (submitting()) {
            <mat-spinner diameter="20"></mat-spinner>
            <span>Initialisation…</span>
          } @else {
            <mat-icon>send</mat-icon>
            <span>Initier paiement</span>
          }
        </button>
      </form>

      @if (result(); as r) {
        <div class="result" role="status" data-testid="momo-result">
          <mat-icon [class]="'icon-' + r.status.toLowerCase()">
            {{ r.status === 'SUCCESS' ? 'check_circle' : 'hourglass_top' }}
          </mat-icon>
          <div>
            <strong>Statut&nbsp;: {{ statusLabel(r.status) }}</strong>
            <p>Transaction&nbsp;: {{ r.txId }}</p>
            @if (r.message) {
              <p class="msg">{{ r.message }}</p>
            }
            @if (r.status === 'PENDING') {
              <p class="hint">
                Un SMS vous sera envoyé pour confirmer le paiement.
                Le paiement est en cours — veuillez patienter.
              </p>
            }
          </div>
        </div>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    .momo {
      max-width: 520px;
      margin: 0 auto;
      padding: 24px 16px 40px;
    }
    .head { margin-bottom: 24px; text-align: center; }
    .head h1 { margin: 0 0 4px; font-size: 1.5rem; }
    .head p { margin: 0; color: #666; font-size: 0.9rem; }
    .form {
      display: flex;
      flex-direction: column;
      gap: 12px;
    }
    .full { width: 100%; }
    .cta {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      justify-content: center;
      padding: 10px 20px;
      min-height: 44px;
    }
    .emoji { margin-right: 6px; font-size: 1.1em; }
    .result {
      margin-top: 24px;
      padding: 16px;
      border-radius: 8px;
      background: #eef7ff;
      border: 1px solid #b8ddf8;
      display: flex;
      gap: 12px;
      align-items: flex-start;
    }
    .result .icon-pending { color: #f57c00; }
    .result .icon-success { color: #2e7d32; }
    .result .icon-failed  { color: #c62828; }
    .result strong { display: block; margin-bottom: 4px; }
    .result p { margin: 0; font-size: 0.9rem; color: #333; }
    .result .msg { margin-top: 6px; font-style: italic; }
    .result .hint { margin-top: 6px; color: #555; }
  `],
})
export class MobileMoneyFormComponent {
  private readonly fb = inject(FormBuilder);
  private readonly route = inject(ActivatedRoute);
  private readonly momo = inject(MobileMoneyService);

  /** Optional input for programmatic use; falls back to route param. */
  readonly txIdInput = input<string | null>(null);

  readonly providers = PROVIDERS;

  readonly referenceId = computed(() => {
    const fromInput = this.txIdInput();
    if (fromInput) return fromInput;
    const fromRoute = this.route.snapshot.paramMap.get('txId');
    return fromRoute ?? `ref-${Date.now()}`;
  });

  readonly submitting = signal(false);
  readonly result = signal<MobileMoneyInitiateResponse | null>(null);

  readonly form = this.fb.group({
    provider: this.fb.nonNullable.control<MobileMoneyProvider>('orange_money', Validators.required),
    phone: new FormControl('', {
      nonNullable: true,
      validators: [Validators.required, Validators.pattern(/^\d{8}$/)],
    }),
    amount: new FormControl<number | null>(null, {
      nonNullable: false,
      validators: [Validators.required, Validators.min(100)],
    }),
  });

  statusLabel(status: MobileMoneyInitiateResponse['status']): string {
    switch (status) {
      case 'PENDING': return 'PENDING — paiement initié, en cours';
      case 'SUCCESS': return 'SUCCESS — paiement confirmé';
      case 'FAILED':  return 'FAILED — paiement refusé';
    }
  }

  onSubmit(): void {
    if (this.form.invalid || this.submitting()) return;
    this.submitting.set(true);

    const { provider, phone, amount } = this.form.getRawValue();
    // Auto-prefix +226 (phone validation already ensures 8 digits).
    const fullPhone = `+226${phone}`;

    this.momo
      .initiate({
        provider: provider!,
        phone: fullPhone,
        amount: amount ?? 0,
        reference: this.referenceId(),
      })
      .subscribe({
        next: (res) => {
          this.result.set(res);
          this.submitting.set(false);
        },
        error: () => {
          this.submitting.set(false);
        },
      });
  }
}
