// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Component, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
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
    MatButtonModule,
    MatIconModule,
    MatProgressSpinnerModule,
    TranslateModule,
  ],
  template: `
    <div class="forgot-page">
      <!-- Panneau gauche : hero -->
      <aside class="hero-panel" aria-hidden="true">
        <img class="hero-bg" src="assets/img/hero-farm-illustration.svg" alt="">
        <div class="hero-overlay"></div>
        <div class="hero-content">
          <a routerLink="/" class="hero-brand">
            <img src="assets/img/logo-poulets-bf.svg" alt="" width="44" height="44">
            <span>Poulets <strong>BF</strong></span>
          </a>
          <h2>Mot de passe oublié ?</h2>
          <p>Pas de panique — on vous envoie un lien de réinitialisation en quelques secondes.</p>
        </div>
      </aside>

      <!-- Panneau droit : formulaire compact -->
      <main class="form-panel">
        <div class="form-inner">
          <a routerLink="/" class="form-brand-mobile">
            <img src="assets/img/logo-poulets-bf.svg" alt="" width="36" height="36">
            <span>Poulets <strong>BF</strong></span>
          </a>

          <header class="form-head">
            <h1>Réinitialiser le mot de passe</h1>
            <p>Indiquez l'email associé à votre compte. Vous recevrez un lien de réinitialisation valable 1 heure.</p>
          </header>

          @if (sent()) {
            <div class="success" role="status">
              <mat-icon>check_circle</mat-icon>
              <div>
                <strong>Lien envoyé</strong>
                <p>Vérifiez votre boîte de réception. Le lien expire dans 1 heure.</p>
              </div>
            </div>
            <a mat-flat-button color="primary" routerLink="/auth/login" class="cta">
              Retour à la connexion
            </a>
          } @else {
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
                    required
                  >
                </div>
                @if (form.get('email')?.hasError('required') && form.get('email')?.touched) {
                  <small class="err-msg">Email requis</small>
                } @else if (form.get('email')?.hasError('email') && form.get('email')?.touched) {
                  <small class="err-msg">Format d'email invalide</small>
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
                  <mat-spinner diameter="22"></mat-spinner> Envoi…
                } @else {
                  Envoyer le lien <mat-icon>arrow_forward</mat-icon>
                }
              </button>
            </form>

            <div class="sep"><span>ou</span></div>

            <a mat-stroked-button routerLink="/auth/login" class="back-cta">
              <mat-icon>arrow_back</mat-icon>
              Retour à la connexion
            </a>
          }

          <footer class="form-foot">
            <p class="copy">© 2026 FASO DIGITALISATION — AGPL-3.0</p>
          </footer>
        </div>
      </main>
    </div>
  `,
  styles: [`
    :host { display: block; }

    @keyframes fp-rise {
      from { opacity: 0; transform: translateY(24px); }
      to   { opacity: 1; transform: translateY(0); }
    }
    @keyframes fp-slide {
      from { opacity: 0; transform: translateX(24px); }
      to   { opacity: 1; transform: translateX(0); }
    }
    @keyframes fp-bg {
      from { transform: scale(1.08); }
      to   { transform: scale(1); }
    }
    .hero-bg { animation: fp-bg 1800ms cubic-bezier(0, 0, 0.2, 1) both; }
    .hero-content > * {
      opacity: 0;
      animation: fp-rise 700ms cubic-bezier(0.2, 0, 0.2, 1) forwards;
    }
    .hero-content .hero-brand { animation-delay: 100ms; }
    .hero-content h2          { animation-delay: 250ms; }
    .hero-content p           { animation-delay: 400ms; }
    .form-inner {
      animation: fp-slide 700ms cubic-bezier(0.2, 0, 0.2, 1) both;
      animation-delay: 200ms;
    }
    @media (prefers-reduced-motion: reduce) {
      .hero-bg, .hero-content > *, .form-inner { animation: none !important; opacity: 1 !important; transform: none !important; }
    }

    .forgot-page {
      display: grid;
      grid-template-columns: 1.1fr 1fr;
      min-height: 100vh;
      background: #FFFFFF;
    }

    /* ============== Hero panneau gauche ============== */
    .hero-panel {
      position: relative;
      overflow: hidden;
      color: #FFFFFF;
      display: flex;
      align-items: flex-end;
      padding: var(--faso-space-10);
    }
    .hero-bg { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; z-index: 0; }
    .hero-overlay {
      position: absolute; inset: 0; z-index: 1;
      background: linear-gradient(180deg, rgba(15, 62, 30, 0.25) 0%, rgba(15, 62, 30, 0.70) 70%, rgba(15, 62, 30, 0.85) 100%);
    }
    .hero-content { position: relative; z-index: 2; max-width: 500px; }
    .hero-brand {
      display: inline-flex; align-items: center; gap: 10px;
      color: #FFFFFF; text-decoration: none;
      font-size: 1.5rem; font-weight: 600;
      margin-bottom: var(--faso-space-10);
      text-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
    }
    .hero-brand strong { color: #FFB300; }
    .hero-content h2 {
      color: #FFFFFF;
      font-size: clamp(1.75rem, 3vw, 2.5rem);
      font-weight: 700; line-height: 1.15;
      margin: 0 0 var(--faso-space-3);
      text-shadow: 0 2px 12px rgba(0, 0, 0, 0.35);
    }
    .hero-content p { margin: 0; font-size: 1.125rem; opacity: 0.95; }

    /* ============== Form panneau droit ============== */
    .form-panel {
      display: flex; align-items: center; justify-content: center;
      padding: var(--faso-space-8) var(--faso-space-6);
      background: #FFFFFF; color: #0F172A;
    }
    .form-inner { width: 100%; max-width: 440px; color: #0F172A; }

    .form-brand-mobile {
      display: none;
      align-items: center; gap: 8px;
      color: #0F172A; text-decoration: none;
      font-size: 1.25rem; font-weight: 600;
      margin-bottom: var(--faso-space-6);
    }
    .form-brand-mobile strong { color: #2E7D32; }

    .form-head { margin-bottom: var(--faso-space-6); }
    .form-head h1 {
      margin: 0 0 var(--faso-space-2);
      font-size: 1.875rem; font-weight: 700;
      color: #0F172A; letter-spacing: -0.015em;
    }
    .form-head p { margin: 0; color: #475569; font-size: 1rem; line-height: 1.55; }

    form { display: flex; flex-direction: column; gap: var(--faso-space-4); }

    .field { display: flex; flex-direction: column; gap: 6px; }
    .lbl { font-size: 0.875rem; font-weight: 600; color: #0F172A; }
    .err-msg { color: #D32F2F; font-size: 0.75rem; font-weight: 500; margin-top: 2px; }

    .input-wrap {
      display: flex; align-items: center;
      background: #FFFFFF;
      border: 1.5px solid #D1D5DB;
      border-radius: 8px;
      padding: 0 12px; gap: 10px;
      transition: border-color 160ms ease, box-shadow 160ms ease;
    }
    .input-wrap:hover { border-color: #66BB6A; }
    .input-wrap:focus-within {
      border-color: #2E7D32;
      box-shadow: 0 0 0 4px rgba(46, 125, 50, 0.14);
    }
    .input-wrap > mat-icon { font-size: 20px; width: 20px; height: 20px; color: #64748B; flex-shrink: 0; }
    .input-wrap input {
      flex: 1; border: none; outline: none; background: transparent;
      padding: 12px 0; font-family: inherit; font-size: 1rem; color: #0F172A; min-width: 0;
    }
    .input-wrap input::placeholder { color: #94A3B8; }

    .field.invalid .input-wrap { border-color: #D32F2F; }

    .cta {
      height: 48px;
      border-radius: 8px !important;
      font-size: 1rem !important;
      font-weight: 600 !important;
      display: inline-flex !important;
      align-items: center; justify-content: center;
      gap: 8px;
      margin-top: var(--faso-space-2);
      width: 100%;
    }
    .cta[disabled] { opacity: 0.55; }

    .sep {
      display: flex; align-items: center; gap: 12px;
      margin: var(--faso-space-5) 0;
      color: #94A3B8;
      font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.08em;
    }
    .sep::before, .sep::after {
      content: ''; flex: 1; height: 1px; background: #E5E7EB;
    }

    .back-cta {
      width: 100%;
      height: 48px;
      border-radius: 8px !important;
      display: inline-flex !important;
      align-items: center; justify-content: center;
      gap: 8px;
      color: #1B5E20 !important;
      border-color: #A5D6A7 !important;
    }

    .success {
      display: flex; gap: 12px; align-items: flex-start;
      padding: 16px;
      background: #E8F5E9;
      border: 1px solid #2E7D32;
      border-radius: 8px;
      margin-bottom: var(--faso-space-4);
    }
    .success mat-icon {
      font-size: 28px; width: 28px; height: 28px;
      color: #2E7D32; flex-shrink: 0;
    }
    .success strong { display: block; color: #0F172A; }
    .success p { margin: 4px 0 0; color: #475569; font-size: 0.875rem; }

    .form-foot {
      margin-top: var(--faso-space-8);
      padding-top: var(--faso-space-4);
      border-top: 1px solid #E5E7EB;
      text-align: center;
    }
    .form-foot .copy { color: #94A3B8; font-size: 0.75rem; margin: 0; }

    /* ============== Responsive ============== */
    @media (max-width: 899px) {
      .forgot-page { grid-template-columns: 1fr; }
      .hero-panel { min-height: 200px; padding: var(--faso-space-6); }
      .hero-content h2 { font-size: 1.5rem; }
      .hero-content p { font-size: 1rem; }
    }
    @media (max-width: 639px) {
      .hero-panel { display: none; }
      .form-panel { padding: var(--faso-space-6) var(--faso-space-4); }
      .form-brand-mobile { display: inline-flex; }
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

  emailTouchedInvalid(): boolean {
    const c = this.form.get('email');
    return !!c && c.touched && c.invalid;
  }

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
