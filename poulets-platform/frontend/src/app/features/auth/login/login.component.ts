// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Component, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink, Router, ActivatedRoute } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
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
    MatButtonModule,
    MatIconModule,
    MatProgressSpinnerModule,
    TranslateModule,
  ],
  template: `
    <div class="login-page">
      <!-- Left panel : brand + hero + value prop -->
      <aside class="hero-panel" aria-hidden="true">
        <img class="hero-bg" src="assets/img/hero-farm-illustration.svg" alt="">
        <div class="hero-overlay"></div>
        <div class="hero-content">
          <a routerLink="/" class="hero-brand">
            <img src="assets/img/logo-poulets-bf.svg" alt="" width="44" height="44">
            <span>Poulets <strong>BF</strong></span>
          </a>
          <h2>Des poulets, direct de l'éleveur burkinabè.</h2>
          <p>Halal · traçable · livré au bon moment.</p>
          <ul class="hero-points">
            <li><span class="dot"></span> 200+ éleveurs vérifiés</li>
            <li><span class="dot"></span> 13 régions du Burkina Faso</li>
            <li><span class="dot"></span> Paiement mobile money sécurisé</li>
          </ul>
        </div>
      </aside>

      <!-- Right panel : form card -->
      <main class="form-panel">
        <div class="form-inner">
          <a routerLink="/" class="form-brand-mobile">
            <img src="assets/img/logo-poulets-bf.svg" alt="" width="36" height="36">
            <span>Poulets <strong>BF</strong></span>
          </a>

          <header class="form-head">
            <h1>Connexion</h1>
            <p>Accédez à votre compte éleveur, client ou administrateur.</p>
          </header>

          @if (errorMessage()) {
            <div class="error" role="alert">
              <mat-icon>error_outline</mat-icon>
              <span>{{ errorMessage() | translate }}</span>
            </div>
          }

          <form [formGroup]="form" (ngSubmit)="onSubmit()" novalidate>
            <label class="field" [class.invalid]="emailTouchedInvalid()">
              <span class="lbl">Adresse email</span>
              <div class="input-wrap">
                <mat-icon aria-hidden="true">mail</mat-icon>
                <input
                  type="email"
                  formControlName="email"
                  autocomplete="email"
                  inputmode="email"
                  placeholder="vous&#64;exemple.bf"
                  [attr.aria-invalid]="emailTouchedInvalid() || null"
                >
              </div>
              @if (form.get('email')?.hasError('required') && form.get('email')?.touched) {
                <small class="err-msg">Email requis</small>
              } @else if (form.get('email')?.hasError('email') && form.get('email')?.touched) {
                <small class="err-msg">Format d'email invalide</small>
              }
            </label>

            <label class="field" [class.invalid]="passwordTouchedInvalid()">
              <div class="lbl-row">
                <span class="lbl">Mot de passe</span>
                <a routerLink="/auth/forgot-password" class="forgot">Mot de passe oublié&nbsp;?</a>
              </div>
              <div class="input-wrap">
                <mat-icon aria-hidden="true">lock</mat-icon>
                <input
                  [type]="hidePassword() ? 'password' : 'text'"
                  formControlName="password"
                  autocomplete="current-password"
                  placeholder="••••••••"
                  [attr.aria-invalid]="passwordTouchedInvalid() || null"
                >
                <button
                  type="button"
                  class="toggle-pw"
                  (click)="hidePassword.set(!hidePassword())"
                  [attr.aria-label]="hidePassword() ? 'Afficher le mot de passe' : 'Masquer le mot de passe'"
                >
                  <mat-icon>{{ hidePassword() ? 'visibility_off' : 'visibility' }}</mat-icon>
                </button>
              </div>
              @if (form.get('password')?.hasError('required') && form.get('password')?.touched) {
                <small class="err-msg">Mot de passe requis</small>
              }
            </label>

            <button
              mat-flat-button
              color="primary"
              type="submit"
              [disabled]="form.invalid || loading()"
              class="cta"
            >
              @if (loading()) {
                <mat-spinner diameter="22"></mat-spinner>
                <span>Connexion…</span>
              } @else {
                <span>Se connecter</span>
                <mat-icon>arrow_forward</mat-icon>
              }
            </button>
          </form>

          <div class="sep"><span>ou</span></div>

          <a mat-stroked-button routerLink="/auth/register" class="register-cta">
            <mat-icon>person_add</mat-icon>
            Créer un nouveau compte
          </a>

          <footer class="form-foot">
            <p>En vous connectant, vous acceptez nos <a href="#">conditions</a> et notre <a href="#">politique de confidentialité</a>.</p>
            <p class="copy">© 2026 FASO DIGITALISATION — AGPL-3.0</p>
          </footer>
        </div>
      </main>
    </div>
  `,
  styles: [`
    :host { display: block; }

    /* Animations d'entrée */
    @keyframes login-hero-rise {
      from { opacity: 0; transform: translateY(24px); }
      to   { opacity: 1; transform: translateY(0); }
    }
    @keyframes login-form-slide {
      from { opacity: 0; transform: translateX(24px); }
      to   { opacity: 1; transform: translateX(0); }
    }
    @keyframes login-bg-zoom {
      from { transform: scale(1.1); }
      to   { transform: scale(1); }
    }
    .hero-bg { animation: login-bg-zoom 1800ms cubic-bezier(0, 0, 0.2, 1) both; }
    .hero-content > * {
      opacity: 0;
      animation: login-hero-rise 700ms cubic-bezier(0.2, 0, 0.2, 1) forwards;
    }
    .hero-content .hero-brand { animation-delay: 100ms; }
    .hero-content h2           { animation-delay: 250ms; }
    .hero-content .lead        { animation-delay: 400ms; }
    .hero-content .hero-hints  { animation-delay: 550ms; }
    .form-inner {
      animation: login-form-slide 700ms cubic-bezier(0.2, 0, 0.2, 1) both;
      animation-delay: 200ms;
    }
    @media (prefers-reduced-motion: reduce) {
      .hero-bg, .hero-content > *, .form-inner { animation: none !important; opacity: 1 !important; transform: none !important; }
    }

    .login-page {
      display: grid;
      grid-template-columns: 1.1fr 1fr;
      min-height: 100vh;
      background: var(--faso-bg);
    }

    /* ============== Left hero panel ============== */
    .hero-panel {
      position: relative;
      overflow: hidden;
      color: #FFFFFF;
      display: flex;
      align-items: flex-end;
      padding: var(--faso-space-10);
    }
    .hero-bg {
      position: absolute;
      inset: 0;
      width: 100%;
      height: 100%;
      object-fit: cover;
      z-index: 0;
    }
    .hero-overlay {
      position: absolute;
      inset: 0;
      z-index: 1;
      background:
        linear-gradient(180deg, rgba(15, 62, 30, 0.25) 0%, rgba(15, 62, 30, 0.70) 70%, rgba(15, 62, 30, 0.85) 100%);
    }
    .hero-content {
      position: relative;
      z-index: 2;
      max-width: 500px;
    }
    .hero-brand {
      display: inline-flex;
      align-items: center;
      gap: 10px;
      color: #FFFFFF;
      text-decoration: none;
      font-size: 1.5rem;
      font-weight: 600;
      margin-bottom: var(--faso-space-10);
      text-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
    }
    .hero-brand strong { color: var(--faso-accent-400); }
    .hero-content h2 {
      color: #FFFFFF;
      font-size: clamp(1.75rem, 3vw, 2.5rem);
      font-weight: 700;
      line-height: 1.15;
      letter-spacing: -0.015em;
      margin: 0 0 var(--faso-space-3);
      text-shadow: 0 2px 12px rgba(0, 0, 0, 0.35);
    }
    .hero-content p {
      margin: 0 0 var(--faso-space-6);
      font-size: var(--faso-text-lg);
      opacity: 0.92;
    }
    .hero-points {
      list-style: none;
      padding: 0;
      margin: 0;
      display: flex;
      flex-direction: column;
      gap: 10px;
    }
    .hero-points li {
      display: inline-flex;
      align-items: center;
      gap: 10px;
      font-size: var(--faso-text-base);
      opacity: 0.95;
    }
    .hero-points .dot {
      display: inline-block;
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--faso-accent-400);
      box-shadow: 0 0 0 4px rgba(255, 179, 0, 0.25);
    }

    /* ============== Right form panel ============== */
    /* Couleurs FIGÉES (pas de tokens) pour garantir le contraste quelle que soit
       la préférence OS dark/light. Fond blanc → texte slate-900 lisible. */
    .form-panel {
      display: flex;
      align-items: center;
      justify-content: center;
      padding: var(--faso-space-8) var(--faso-space-6);
      background: #FFFFFF;
      color: #0F172A;
    }
    .form-inner {
      width: 100%;
      max-width: 440px;
      color: #0F172A;
    }

    .form-brand-mobile {
      display: none;
      align-items: center;
      gap: 8px;
      color: #0F172A;
      text-decoration: none;
      font-size: 1.25rem;
      font-weight: 600;
      margin-bottom: var(--faso-space-6);
    }
    .form-brand-mobile strong { color: #2E7D32; }

    .form-head { margin-bottom: var(--faso-space-6); }
    .form-head h1 {
      margin: 0 0 var(--faso-space-2);
      font-size: var(--faso-text-3xl);
      font-weight: 700;
      color: #0F172A;
      letter-spacing: -0.015em;
    }
    .form-head p {
      margin: 0;
      color: #475569;
      font-size: var(--faso-text-base);
      line-height: 1.55;
    }

    .error {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 12px 14px;
      background: var(--faso-danger-bg);
      color: var(--faso-danger);
      border: 1px solid var(--faso-danger);
      border-radius: var(--faso-radius-md);
      margin-bottom: var(--faso-space-4);
      font-size: var(--faso-text-sm);
      font-weight: 500;
    }
    .error mat-icon { font-size: 20px; width: 20px; height: 20px; }

    form { display: flex; flex-direction: column; gap: var(--faso-space-4); }

    .field { display: flex; flex-direction: column; gap: 6px; }
    .lbl-row {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
    }
    .lbl {
      font-size: var(--faso-text-sm);
      font-weight: 600;
      color: #0F172A;
    }
    .forgot {
      font-size: var(--faso-text-sm);
      color: #2E7D32;
      text-decoration: none;
      font-weight: 500;
    }
    .forgot:hover { text-decoration: underline; color: #1B5E20; }

    .input-wrap {
      display: flex;
      align-items: center;
      background: #FFFFFF;
      border: 1.5px solid #D1D5DB;
      border-radius: var(--faso-radius-md);
      padding: 0 12px;
      gap: 10px;
      transition: border-color 160ms ease, box-shadow 160ms ease;
    }
    .input-wrap:hover { border-color: #66BB6A; }
    .input-wrap:focus-within {
      border-color: #2E7D32;
      box-shadow: 0 0 0 4px rgba(46, 125, 50, 0.14);
    }
    .input-wrap > mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
      color: #64748B;
      flex-shrink: 0;
    }
    .input-wrap input {
      flex: 1;
      border: none;
      outline: none;
      background: transparent;
      padding: 12px 0;
      font-family: inherit;
      font-size: var(--faso-text-base);
      color: #0F172A;
      min-width: 0;
    }
    .input-wrap input::placeholder { color: #94A3B8; }
    .input-wrap input:-webkit-autofill {
      box-shadow: 0 0 0 999px #FFFFFF inset;
      -webkit-text-fill-color: #0F172A;
    }
    .toggle-pw {
      background: transparent;
      border: none;
      cursor: pointer;
      padding: 4px;
      border-radius: 50%;
      color: #64748B;
      display: inline-flex;
    }
    .toggle-pw:hover { background: #F3F4F6; color: #1B5E20; }
    .toggle-pw mat-icon { font-size: 20px; width: 20px; height: 20px; }

    .field.invalid .input-wrap {
      border-color: var(--faso-danger);
    }
    .err-msg {
      color: var(--faso-danger);
      font-size: var(--faso-text-xs);
      font-weight: 500;
      margin-top: 2px;
    }

    .cta {
      height: 48px;
      border-radius: var(--faso-radius-md) !important;
      font-size: var(--faso-text-base) !important;
      font-weight: 600 !important;
      margin-top: var(--faso-space-2);
      display: inline-flex !important;
      align-items: center;
      justify-content: center;
      gap: 8px;
    }
    .cta[disabled] { opacity: 0.55; }

    .sep {
      display: flex;
      align-items: center;
      gap: 12px;
      margin: var(--faso-space-5) 0;
      color: #94A3B8;
      font-size: var(--faso-text-xs);
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }
    .sep::before, .sep::after {
      content: '';
      flex: 1;
      height: 1px;
      background: #E5E7EB;
    }

    .register-cta {
      width: 100%;
      height: 48px;
      border-radius: var(--faso-radius-md) !important;
      font-size: var(--faso-text-base) !important;
      display: inline-flex !important;
      align-items: center;
      justify-content: center;
      gap: 8px;
      color: #1B5E20 !important;
      border-color: #A5D6A7 !important;
    }

    .form-foot {
      margin-top: var(--faso-space-8);
      padding-top: var(--faso-space-4);
      border-top: 1px solid #E5E7EB;
      color: #64748B;
      font-size: var(--faso-text-xs);
      text-align: center;
    }
    .form-foot p { margin: 0 0 4px; }
    .form-foot a { color: #1B5E20; }
    .form-foot .copy { color: #94A3B8; }

    /* ============== Responsive ============== */
    @media (max-width: 899px) {
      .login-page { grid-template-columns: 1fr; }
      .hero-panel {
        min-height: 220px;
        padding: var(--faso-space-6);
      }
      .hero-content h2 { font-size: 1.5rem; }
      .hero-content p { font-size: var(--faso-text-base); }
      .hero-points { display: none; }
      .form-brand-mobile { display: none; }
    }
    @media (max-width: 639px) {
      .hero-panel { display: none; }
      .form-panel { padding: var(--faso-space-6) var(--faso-space-4); }
      .form-brand-mobile { display: inline-flex; }
    }

    /* ============== Dark theme ============== */
    :host-context([data-theme="dark"]) .form-panel { background: var(--faso-surface); }
    :host-context([data-theme="dark"]) .input-wrap {
      background: var(--faso-surface-alt);
      border-color: var(--faso-border);
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

  emailTouchedInvalid(): boolean {
    const c = this.form.get('email');
    return !!c && c.touched && c.invalid;
  }

  passwordTouchedInvalid(): boolean {
    const c = this.form.get('password');
    return !!c && c.touched && c.invalid;
  }

  onSubmit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }

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
