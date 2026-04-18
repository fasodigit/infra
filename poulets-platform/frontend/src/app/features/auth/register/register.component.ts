// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Component, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink, Router } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, Validators, AbstractControl, ValidationErrors } from '@angular/forms';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatSelectModule } from '@angular/material/select';
import { MatStepperModule } from '@angular/material/stepper';
import { MatRadioModule } from '@angular/material/radio';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { TranslateModule } from '@ngx-translate/core';

import { AuthService } from '@core/services/auth.service';
import { Role } from '@app/shared/models/user.model';

@Component({
  selector: 'app-register',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    ReactiveFormsModule,
    MatFormFieldModule,
    MatInputModule,
    MatButtonModule,
    MatIconModule,
    MatSelectModule,
    MatStepperModule,
    MatRadioModule,
    MatProgressSpinnerModule,
    TranslateModule,
  ],
  template: `
    <div class="register-page">
      <!-- Panneau gauche : hero -->
      <aside class="hero-panel" aria-hidden="true">
        <img class="hero-bg" src="assets/img/hero-farm-illustration.svg" alt="">
        <div class="hero-overlay"></div>
        <div class="hero-content">
          <a routerLink="/" class="hero-brand">
            <img src="assets/img/logo-poulets-bf.svg" alt="" width="44" height="44">
            <span>Poulets <strong>BF</strong></span>
          </a>
          <h2>Rejoignez la marketplace souveraine.</h2>
          <p>Éleveur, client, producteur ou admin — chaque compte est vérifié et sécurisé.</p>
          <ul class="hero-points">
            <li><span class="dot"></span> Création de compte gratuite</li>
            <li><span class="dot"></span> Vérification par email</li>
            <li><span class="dot"></span> Sécurité MFA optionnelle</li>
          </ul>
        </div>
      </aside>

      <!-- Panneau droit : formulaire -->
      <main class="form-panel">
        <div class="form-inner">
          <a routerLink="/" class="form-brand-mobile">
            <img src="assets/img/logo-poulets-bf.svg" alt="" width="36" height="36">
            <span>Poulets <strong>BF</strong></span>
          </a>

          <header class="form-head">
            <h1>Créer un compte</h1>
            <p>4 étapes rapides — vous recevrez un email de vérification.</p>
          </header>

          @if (errorMessage()) {
            <div class="error" role="alert">
              <mat-icon>error_outline</mat-icon>
              <span>{{ errorMessage() | translate }}</span>
            </div>
          }

          <mat-stepper [linear]="true" #stepper>
            <!-- Étape 1 : Compte -->
            <mat-step [stepControl]="accountForm" label="Compte">
              <form [formGroup]="accountForm" class="step-form">
                <label class="field">
                  <span class="lbl">Nom complet</span>
                  <div class="input-wrap">
                    <mat-icon aria-hidden="true">person</mat-icon>
                    <input type="text" formControlName="nom" autocomplete="name" placeholder="Prénom Nom" required>
                  </div>
                </label>

                <label class="field">
                  <span class="lbl">Email</span>
                  <div class="input-wrap">
                    <mat-icon aria-hidden="true">mail</mat-icon>
                    <input type="email" formControlName="email" autocomplete="email" inputmode="email" placeholder="vous&#64;exemple.bf" required>
                  </div>
                </label>

                <label class="field">
                  <span class="lbl">Téléphone <small>(facultatif)</small></span>
                  <div class="input-wrap">
                    <mat-icon aria-hidden="true">phone</mat-icon>
                    <input type="tel" formControlName="phone" autocomplete="tel" placeholder="+226 70 12 34 56">
                  </div>
                </label>

                <label class="field">
                  <span class="lbl">Mot de passe</span>
                  <div class="input-wrap">
                    <mat-icon aria-hidden="true">lock</mat-icon>
                    <input [type]="hidePassword() ? 'password' : 'text'" formControlName="password" autocomplete="new-password" placeholder="8 caractères minimum" required>
                    <button type="button" class="toggle-pw" (click)="hidePassword.set(!hidePassword())" aria-label="Afficher/masquer">
                      <mat-icon>{{ hidePassword() ? 'visibility_off' : 'visibility' }}</mat-icon>
                    </button>
                  </div>
                </label>

                <label class="field">
                  <span class="lbl">Confirmer le mot de passe</span>
                  <div class="input-wrap">
                    <mat-icon aria-hidden="true">lock</mat-icon>
                    <input [type]="hidePassword() ? 'password' : 'text'" formControlName="confirmPassword" autocomplete="new-password" placeholder="Retapez le mot de passe" required>
                  </div>
                  @if (accountForm.get('confirmPassword')?.hasError('passwordMismatch') && accountForm.get('confirmPassword')?.touched) {
                    <small class="err-msg">Les mots de passe ne correspondent pas</small>
                  }
                </label>

                <div class="step-actions">
                  <span></span>
                  <button
                    mat-flat-button
                    color="primary"
                    matStepperNext
                    type="button"
                    class="cta"
                    [disabled]="accountForm.invalid"
                  >
                    Continuer <mat-icon>arrow_forward</mat-icon>
                  </button>
                </div>
              </form>
            </mat-step>

            <!-- Étape 2 : Rôle -->
            <mat-step [stepControl]="roleForm" label="Rôle">
              <form [formGroup]="roleForm" class="step-form">
                <p class="step-hint">Choisissez le profil qui correspond à votre activité.</p>
                <div class="role-grid">
                  <label class="role-card" [class.selected]="roleForm.value.role === 'eleveur'">
                    <input type="radio" formControlName="role" value="eleveur">
                    <mat-icon>agriculture</mat-icon>
                    <strong>Éleveur</strong>
                    <small>Vends tes poulets, gère tes lots et certifications halal</small>
                  </label>
                  <label class="role-card" [class.selected]="roleForm.value.role === 'client'">
                    <input type="radio" formControlName="role" value="client">
                    <mat-icon>shopping_cart</mat-icon>
                    <strong>Client</strong>
                    <small>Achète des poulets, suis tes commandes, évalue les éleveurs</small>
                  </label>
                  <label class="role-card" [class.selected]="roleForm.value.role === 'producteur_aliment'">
                    <input type="radio" formControlName="role" value="producteur_aliment">
                    <mat-icon>factory</mat-icon>
                    <strong>Producteur</strong>
                    <small>Aliments, poussins, produits vétérinaires pour éleveurs</small>
                  </label>
                </div>

                <div class="step-actions">
                  <button mat-button matStepperPrevious type="button">← Précédent</button>
                  <button mat-flat-button color="primary" matStepperNext type="button" class="cta">
                    Continuer <mat-icon>arrow_forward</mat-icon>
                  </button>
                </div>
              </form>
            </mat-step>

            <!-- Étape 3 : Détails spécifiques au rôle -->
            <mat-step label="Détails" [optional]="true">
              <form [formGroup]="detailsForm" class="step-form">
                <label class="field">
                  <span class="lbl">Localisation</span>
                  <div class="input-wrap">
                    <mat-icon aria-hidden="true">location_on</mat-icon>
                    <input type="text" formControlName="localisation" placeholder="Ville, région">
                  </div>
                </label>

                @if (selectedRole() === 'eleveur') {
                  <label class="field">
                    <span class="lbl">Capacité d'élevage (nb poulets)</span>
                    <div class="input-wrap">
                      <mat-icon aria-hidden="true">pets</mat-icon>
                      <input type="number" formControlName="capacite" placeholder="100" min="1">
                    </div>
                  </label>
                }

                @if (selectedRole() === 'client') {
                  <label class="field">
                    <span class="lbl">Type de client</span>
                    <div class="input-wrap">
                      <mat-icon aria-hidden="true">groups</mat-icon>
                      <select formControlName="clientType">
                        <option value="">Sélectionner</option>
                        <option value="PARTICULIER">Particulier</option>
                        <option value="RESTAURANT">Restaurant / hôtel</option>
                        <option value="REVENDEUR">Revendeur / grossiste</option>
                      </select>
                    </div>
                  </label>
                }

                @if (selectedRole() === 'producteur_aliment') {
                  <label class="field">
                    <span class="lbl">Zone de distribution</span>
                    <div class="input-wrap">
                      <mat-icon aria-hidden="true">map</mat-icon>
                      <input type="text" formControlName="zoneDistribution" placeholder="Régions livrées">
                    </div>
                  </label>
                }

                <div class="step-actions">
                  <button mat-button matStepperPrevious type="button">← Précédent</button>
                  <button mat-flat-button color="primary" matStepperNext type="button" class="cta">
                    Continuer <mat-icon>arrow_forward</mat-icon>
                  </button>
                </div>
              </form>
            </mat-step>

            <!-- Étape 4 : Groupement -->
            <mat-step label="Groupement" [optional]="true">
              <form [formGroup]="groupementForm" class="step-form">
                <p class="step-hint">Si vous appartenez à un groupement ou coopérative, indiquez-le ici.</p>
                <label class="field">
                  <span class="lbl">Nom du groupement <small>(facultatif)</small></span>
                  <div class="input-wrap">
                    <mat-icon aria-hidden="true">groups</mat-icon>
                    <input type="text" formControlName="groupementNom" placeholder="Coopérative des éleveurs du Kadiogo">
                  </div>
                </label>

                <div class="step-actions">
                  <button mat-button matStepperPrevious type="button">← Précédent</button>
                  <button mat-flat-button color="primary" type="button" (click)="onSubmit()" [disabled]="loading()" class="cta">
                    @if (loading()) {
                      <mat-spinner diameter="20"></mat-spinner> Création…
                    } @else {
                      Créer mon compte <mat-icon>check</mat-icon>
                    }
                  </button>
                </div>
              </form>
            </mat-step>
          </mat-stepper>

          <footer class="form-foot">
            <p>Déjà inscrit ? <a routerLink="/auth/login">Se connecter</a></p>
            <p class="copy">© 2026 FASO DIGITALISATION — AGPL-3.0</p>
          </footer>
        </div>
      </main>
    </div>
  `,
  styles: [`
    :host { display: block; }

    /* Animations entrée */
    @keyframes reg-rise {
      from { opacity: 0; transform: translateY(24px); }
      to   { opacity: 1; transform: translateY(0); }
    }
    @keyframes reg-slide-in {
      from { opacity: 0; transform: translateX(24px); }
      to   { opacity: 1; transform: translateX(0); }
    }
    @keyframes reg-bg-zoom {
      from { transform: scale(1.08); }
      to   { transform: scale(1); }
    }
    .hero-bg { animation: reg-bg-zoom 1800ms cubic-bezier(0, 0, 0.2, 1) both; }
    .hero-content > * {
      opacity: 0;
      animation: reg-rise 700ms cubic-bezier(0.2, 0, 0.2, 1) forwards;
    }
    .hero-content .hero-brand  { animation-delay: 100ms; }
    .hero-content h2           { animation-delay: 250ms; }
    .hero-content p            { animation-delay: 400ms; }
    .hero-content .hero-points { animation-delay: 550ms; }
    .form-inner {
      animation: reg-slide-in 700ms cubic-bezier(0.2, 0, 0.2, 1) both;
      animation-delay: 200ms;
    }
    .role-card {
      transition: transform 240ms cubic-bezier(0, 0, 0.2, 1),
                  border-color 240ms, background 240ms, box-shadow 240ms;
    }
    .role-card:hover {
      transform: translateY(-3px);
      box-shadow: 0 8px 20px rgba(46, 125, 50, 0.15);
    }
    @media (prefers-reduced-motion: reduce) {
      .hero-bg, .hero-content > *, .form-inner, .role-card { animation: none !important; opacity: 1 !important; transform: none !important; }
    }

    .register-page {
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
    .hero-bg {
      position: absolute; inset: 0;
      width: 100%; height: 100%; object-fit: cover; z-index: 0;
    }
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
      font-weight: 700; line-height: 1.15; letter-spacing: -0.015em;
      margin: 0 0 var(--faso-space-3);
      text-shadow: 0 2px 12px rgba(0, 0, 0, 0.35);
    }
    .hero-content p {
      margin: 0 0 var(--faso-space-6);
      font-size: 1.125rem; opacity: 0.95;
    }
    .hero-points { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 10px; }
    .hero-points li { display: inline-flex; align-items: center; gap: 10px; opacity: 0.95; }
    .hero-points .dot {
      display: inline-block; width: 8px; height: 8px; border-radius: 50%;
      background: #FFB300; box-shadow: 0 0 0 4px rgba(255, 179, 0, 0.25);
    }

    /* ============== Form panneau droit ============== */
    .form-panel {
      display: flex; align-items: center; justify-content: center;
      padding: var(--faso-space-8) var(--faso-space-6);
      background: #FFFFFF; color: #0F172A;
    }
    .form-inner { width: 100%; max-width: 520px; color: #0F172A; }

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

    .error {
      display: flex; align-items: center; gap: 8px;
      padding: 12px 14px;
      background: #FDECEC; color: #D32F2F; border: 1px solid #D32F2F;
      border-radius: 8px; margin-bottom: var(--faso-space-4);
      font-size: 0.9rem; font-weight: 500;
    }
    .error mat-icon { font-size: 20px; width: 20px; height: 20px; }

    .step-form {
      display: flex; flex-direction: column; gap: var(--faso-space-4);
      padding: var(--faso-space-4) 0 0;
    }
    .step-hint { color: #475569; margin: 0; }

    /* ── Champs personnalisés (pas Material mat-form-field) ── */
    .field { display: flex; flex-direction: column; gap: 6px; }
    .lbl { font-size: 0.875rem; font-weight: 600; color: #0F172A; }
    .lbl small { color: #94A3B8; font-weight: 400; }
    .err-msg { color: #D32F2F; font-size: 0.75rem; font-weight: 500; }

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
    .input-wrap > mat-icon {
      font-size: 20px; width: 20px; height: 20px;
      color: #64748B; flex-shrink: 0;
    }
    .input-wrap input,
    .input-wrap select {
      flex: 1; border: none; outline: none;
      background: transparent;
      padding: 12px 0;
      font-family: inherit; font-size: 1rem;
      color: #0F172A; min-width: 0;
    }
    .input-wrap input::placeholder { color: #94A3B8; }

    .toggle-pw {
      background: transparent; border: none; cursor: pointer;
      padding: 4px; border-radius: 50%; color: #64748B;
      display: inline-flex;
    }
    .toggle-pw:hover { background: #F3F4F6; color: #1B5E20; }
    .toggle-pw mat-icon { font-size: 20px; width: 20px; height: 20px; }

    /* ── Cards de rôle ── */
    .role-grid {
      display: grid; gap: var(--faso-space-2);
    }
    .role-card {
      display: grid;
      grid-template-columns: auto auto 1fr;
      grid-template-rows: auto auto;
      gap: 4px 12px;
      align-items: center;
      padding: var(--faso-space-4);
      background: #FFFFFF;
      border: 1.5px solid #D1D5DB;
      border-radius: 12px;
      cursor: pointer;
      transition: border-color 160ms, background 160ms;
    }
    .role-card:hover { border-color: #66BB6A; }
    .role-card.selected {
      border-color: #2E7D32;
      background: #E8F5E9;
    }
    .role-card input[type="radio"] {
      grid-row: 1 / 3;
      width: 18px; height: 18px; margin: 0;
      accent-color: #2E7D32;
    }
    .role-card mat-icon {
      grid-row: 1 / 3; grid-column: 2;
      font-size: 28px; width: 28px; height: 28px;
      color: #2E7D32;
    }
    .role-card strong {
      grid-row: 1; grid-column: 3;
      font-size: 1rem; color: #0F172A;
    }
    .role-card small {
      grid-row: 2; grid-column: 3;
      color: #475569; font-size: 0.875rem;
    }

    /* ── Actions stepper ── */
    .step-actions {
      display: flex; justify-content: space-between; align-items: center;
      margin-top: var(--faso-space-2);
    }
    .cta {
      height: 44px;
      border-radius: 8px !important;
      font-weight: 600 !important;
      display: inline-flex !important;
      align-items: center; justify-content: center;
      gap: 6px;
    }

    /* ── Material stepper overrides (contraste) ── */
    ::ng-deep .mat-step-header .mat-step-label {
      color: #475569 !important;
    }
    ::ng-deep .mat-step-header .mat-step-label-selected,
    ::ng-deep .mat-step-header .mat-step-label-active {
      color: #0F172A !important;
      font-weight: 600 !important;
    }
    ::ng-deep .mat-stepper-horizontal {
      background: transparent !important;
    }

    .form-foot {
      margin-top: var(--faso-space-6);
      padding-top: var(--faso-space-4);
      border-top: 1px solid #E5E7EB;
      color: #475569; font-size: 0.875rem; text-align: center;
    }
    .form-foot p { margin: 0 0 4px; }
    .form-foot a { color: #2E7D32; font-weight: 500; text-decoration: none; }
    .form-foot a:hover { text-decoration: underline; }
    .form-foot .copy { color: #94A3B8; font-size: 0.75rem; }

    /* ============== Responsive ============== */
    @media (max-width: 899px) {
      .register-page { grid-template-columns: 1fr; }
      .hero-panel { min-height: 220px; padding: var(--faso-space-6); }
      .hero-content h2 { font-size: 1.5rem; }
      .hero-content p { font-size: 1rem; }
      .hero-points { display: none; }
    }
    @media (max-width: 639px) {
      .hero-panel { display: none; }
      .form-panel { padding: var(--faso-space-6) var(--faso-space-4); }
      .form-brand-mobile { display: inline-flex; }
    }
  `],
})
export class RegisterComponent {
  private readonly auth = inject(AuthService);
  private readonly router = inject(Router);
  private readonly fb = inject(FormBuilder);

  readonly loading = signal(false);
  readonly errorMessage = signal('');
  readonly selectedRole = signal<Role>('client');
  readonly hidePassword = signal(true);

  readonly accountForm = this.fb.nonNullable.group({
    nom: ['', Validators.required],
    email: ['', [Validators.required, Validators.email]],
    phone: [''],
    password: ['', [Validators.required, Validators.minLength(8)]],
    confirmPassword: ['', [Validators.required]],
  }, { validators: [this.passwordMatchValidator] });

  readonly roleForm = this.fb.nonNullable.group({
    role: ['client' as Role, Validators.required],
  });

  readonly detailsForm = this.fb.group({
    localisation: [''],
    capacite: [null as number | null],
    clientType: [''],
    zoneDistribution: [''],
  });

  readonly groupementForm = this.fb.group({
    groupementNom: [''],
  });

  constructor() {
    this.roleForm.get('role')!.valueChanges.subscribe((role) => {
      this.selectedRole.set(role as Role);
    });
  }

  passwordMatchValidator(control: AbstractControl): ValidationErrors | null {
    const password = control.get('password');
    const confirmPassword = control.get('confirmPassword');
    if (password && confirmPassword && password.value !== confirmPassword.value) {
      confirmPassword.setErrors({ passwordMismatch: true });
      return { passwordMismatch: true };
    }
    return null;
  }

  onSubmit(): void {
    this.loading.set(true);
    this.errorMessage.set('');

    const account = this.accountForm.getRawValue();
    const role = this.roleForm.getRawValue().role;
    const details = this.detailsForm.getRawValue();
    const groupement = this.groupementForm.getRawValue();

    this.auth.register({
      email: account.email,
      password: account.password,
      nom: account.nom,
      phone: account.phone || undefined,
      role: role as Role,
      localisation: details.localisation || undefined,
      capacite: details.capacite ?? undefined,
      clientType: (details.clientType as any) || undefined,
      zoneDistribution: details.zoneDistribution || undefined,
      groupementNom: groupement.groupementNom || undefined,
    }).subscribe({
      next: () => {
        this.loading.set(false);
        this.auth.navigateByRole();
      },
      error: () => {
        this.loading.set(false);
        this.errorMessage.set('auth.register_error');
      },
    });
  }
}
