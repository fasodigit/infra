import { Component, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { TranslateModule } from '@ngx-translate/core';

import { AuthService } from '@core/services/auth.service';

@Component({
  selector: 'app-forgot-password',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    ReactiveFormsModule,
    MatCardModule,
    MatFormFieldModule,
    MatInputModule,
    MatButtonModule,
    MatIconModule,
    MatProgressSpinnerModule,
    TranslateModule,
  ],
  template: `
    <div class="forgot-page">
      <mat-card class="forgot-card">
        <mat-card-header>
          <mat-card-title class="forgot-title">
            <span class="brand">Poulets BF</span>
            <span class="subtitle">{{ 'auth.reset_password' | translate }}</span>
          </mat-card-title>
        </mat-card-header>

        <mat-card-content>
          @if (sent()) {
            <div class="success-banner">
              <mat-icon>check_circle</mat-icon>
              <span>{{ 'auth.reset_sent' | translate }}</span>
            </div>
            <div class="form-actions">
              <a mat-raised-button color="primary" routerLink="/auth/login" class="full-width">
                {{ 'auth.back_to_login' | translate }}
              </a>
            </div>
          } @else {
            <form [formGroup]="form" (ngSubmit)="onSubmit()">
              <mat-form-field appearance="outline" class="full-width">
                <mat-label>{{ 'auth.email' | translate }}</mat-label>
                <input matInput type="email" formControlName="email" autocomplete="email">
                <mat-icon matPrefix>email</mat-icon>
                @if (form.get('email')?.hasError('required') && form.get('email')?.touched) {
                  <mat-error>{{ 'common.required_field' | translate }}</mat-error>
                }
                @if (form.get('email')?.hasError('email') && form.get('email')?.touched) {
                  <mat-error>{{ 'common.invalid_email' | translate }}</mat-error>
                }
              </mat-form-field>

              <div class="form-actions">
                <button
                  mat-raised-button
                  color="primary"
                  type="submit"
                  [disabled]="form.invalid || loading()"
                  class="full-width"
                >
                  @if (loading()) {
                    <mat-spinner diameter="20"></mat-spinner>
                  } @else {
                    {{ 'auth.send_reset_link' | translate }}
                  }
                </button>
              </div>
            </form>

            <div class="back-link">
              <a routerLink="/auth/login">{{ 'auth.back_to_login' | translate }}</a>
            </div>
          }
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .forgot-page {
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      background: linear-gradient(135deg, #1b5e20 0%, #2e7d32 50%, #43a047 100%);
      padding: 24px;
    }

    .forgot-card {
      width: 100%;
      max-width: 440px;
      padding: 32px;
    }

    .forgot-title {
      text-align: center;
      width: 100%;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 8px;
      margin-bottom: 24px;
    }

    .brand {
      font-size: 1.6rem;
      font-weight: 700;
      color: #2e7d32;
    }

    .subtitle {
      font-size: 1.1rem;
      font-weight: 400;
      color: #666;
    }

    .full-width {
      width: 100%;
    }

    .form-actions {
      margin-top: 16px;
    }

    .form-actions button, .form-actions a {
      height: 48px;
      font-size: 1rem;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .success-banner {
      display: flex;
      align-items: center;
      gap: 8px;
      background: #e8f5e9;
      color: #2e7d32;
      padding: 16px;
      border-radius: 8px;
      margin-bottom: 16px;
    }

    .back-link {
      margin-top: 24px;
      text-align: center;
    }

    .back-link a {
      color: #2e7d32;
      text-decoration: none;
      font-size: 0.9rem;
    }

    .back-link a:hover {
      text-decoration: underline;
    }
  `],
})
export class ForgotPasswordComponent {
  private readonly auth = inject(AuthService);
  private readonly fb = inject(FormBuilder);

  readonly loading = signal(false);
  readonly sent = signal(false);

  readonly form = this.fb.nonNullable.group({
    email: ['', [Validators.required, Validators.email]],
  });

  onSubmit(): void {
    if (this.form.invalid) return;

    this.loading.set(true);
    const { email } = this.form.getRawValue();

    this.auth.forgotPassword(email).subscribe({
      next: () => {
        this.loading.set(false);
        this.sent.set(true);
      },
      error: () => {
        // Still show success to prevent email enumeration
        this.loading.set(false);
        this.sent.set(true);
      },
    });
  }
}
