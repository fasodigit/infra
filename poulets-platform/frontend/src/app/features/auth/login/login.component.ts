import { Component, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink, Router, ActivatedRoute } from '@angular/router';
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
  selector: 'app-login',
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
    <div class="login-page">
      <mat-card class="login-card">
        <mat-card-header>
          <mat-card-title class="login-title">
            <span class="brand">Poulets BF</span>
            <span class="subtitle">{{ 'auth.login' | translate }}</span>
          </mat-card-title>
        </mat-card-header>

        <mat-card-content>
          @if (errorMessage()) {
            <div class="error-banner">
              <mat-icon>error_outline</mat-icon>
              <span>{{ errorMessage() | translate }}</span>
            </div>
          }

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

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'auth.password' | translate }}</mat-label>
              <input
                matInput
                [type]="hidePassword() ? 'password' : 'text'"
                formControlName="password"
                autocomplete="current-password"
              >
              <mat-icon matPrefix>lock</mat-icon>
              <button
                mat-icon-button
                matSuffix
                type="button"
                (click)="hidePassword.set(!hidePassword())"
              >
                <mat-icon>{{ hidePassword() ? 'visibility_off' : 'visibility' }}</mat-icon>
              </button>
              @if (form.get('password')?.hasError('required') && form.get('password')?.touched) {
                <mat-error>{{ 'common.required_field' | translate }}</mat-error>
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
                  {{ 'auth.sign_in' | translate }}
                }
              </button>
            </div>
          </form>

          <div class="login-links">
            <a routerLink="/auth/forgot-password" class="forgot-link">
              {{ 'auth.forgot_password' | translate }}
            </a>
            <div class="register-link">
              {{ 'auth.no_account' | translate }}
              <a routerLink="/auth/register">{{ 'auth.sign_up' | translate }}</a>
            </div>
          </div>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .login-page {
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      background: linear-gradient(135deg, #1b5e20 0%, #2e7d32 50%, #43a047 100%);
      padding: 24px;
    }

    .login-card {
      width: 100%;
      max-width: 440px;
      padding: 32px;
    }

    .login-title {
      text-align: center;
      width: 100%;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 8px;
      margin-bottom: 24px;
    }

    .brand {
      font-size: 1.8rem;
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

    .error-banner {
      display: flex;
      align-items: center;
      gap: 8px;
      background: #fce4ec;
      color: #c62828;
      padding: 12px 16px;
      border-radius: 8px;
      margin-bottom: 16px;
      font-size: 0.9rem;
    }

    .form-actions {
      margin-top: 16px;
    }

    .form-actions button {
      height: 48px;
      font-size: 1rem;
    }

    .login-links {
      margin-top: 24px;
      text-align: center;
    }

    .forgot-link {
      color: #2e7d32;
      font-size: 0.9rem;
      text-decoration: none;
    }

    .forgot-link:hover {
      text-decoration: underline;
    }

    .register-link {
      margin-top: 16px;
      font-size: 0.9rem;
      color: #666;
    }

    .register-link a {
      color: #2e7d32;
      font-weight: 500;
      text-decoration: none;
    }

    .register-link a:hover {
      text-decoration: underline;
    }
  `],
})
export class LoginComponent {
  private readonly auth = inject(AuthService);
  private readonly router = inject(Router);
  private readonly route = inject(ActivatedRoute);
  private readonly fb = inject(FormBuilder);

  readonly loading = signal(false);
  readonly errorMessage = signal('');
  readonly hidePassword = signal(true);

  readonly form = this.fb.nonNullable.group({
    email: ['', [Validators.required, Validators.email]],
    password: ['', [Validators.required]],
  });

  onSubmit(): void {
    if (this.form.invalid) return;

    this.loading.set(true);
    this.errorMessage.set('');

    const { email, password } = this.form.getRawValue();

    this.auth.login({ email, password }).subscribe({
      next: () => {
        this.loading.set(false);
        const returnUrl = this.route.snapshot.queryParams['returnUrl'];
        if (returnUrl) {
          this.router.navigateByUrl(returnUrl);
        } else {
          this.auth.navigateByRole();
        }
      },
      error: () => {
        this.loading.set(false);
        this.errorMessage.set('auth.login_error');
      },
    });
  }
}
