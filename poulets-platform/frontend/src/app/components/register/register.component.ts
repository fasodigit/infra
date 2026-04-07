import { Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { Router, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatSelectModule } from '@angular/material/select';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';

import { AuthService } from '@services/auth.service';

@Component({
  selector: 'app-register',
  standalone: true,
  imports: [
    CommonModule,
    ReactiveFormsModule,
    RouterLink,
    MatCardModule,
    MatFormFieldModule,
    MatInputModule,
    MatButtonModule,
    MatIconModule,
    MatSelectModule,
    MatProgressSpinnerModule,
    MatSnackBarModule,
  ],
  template: `
    <div class="register-container">
      <mat-card class="register-card">
        <mat-card-header>
          <mat-card-title>Inscription</mat-card-title>
          <mat-card-subtitle>Creez votre compte sur Poulets Platform</mat-card-subtitle>
        </mat-card-header>

        <mat-card-content>
          <form [formGroup]="form" (ngSubmit)="onSubmit()">
            <mat-form-field appearance="outline" class="full-width">
              <mat-label>Nom complet</mat-label>
              <input matInput formControlName="name" placeholder="Ouedraogo Ibrahim" />
              <mat-icon matPrefix>person</mat-icon>
              @if (form.controls.name.hasError('required')) {
                <mat-error>Le nom est requis</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>Adresse email</mat-label>
              <input matInput type="email" formControlName="email"
                     placeholder="votre@email.bf" autocomplete="email" />
              <mat-icon matPrefix>email</mat-icon>
              @if (form.controls.email.hasError('required')) {
                <mat-error>L'email est requis</mat-error>
              }
              @if (form.controls.email.hasError('email')) {
                <mat-error>Format d'email invalide</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>Telephone</mat-label>
              <input matInput formControlName="phone" placeholder="+226 70 00 00 00" />
              <mat-icon matPrefix>phone</mat-icon>
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>Mot de passe</mat-label>
              <input matInput [type]="hidePassword() ? 'password' : 'text'"
                     formControlName="password" autocomplete="new-password" />
              <mat-icon matPrefix>lock</mat-icon>
              <button mat-icon-button matSuffix type="button"
                      (click)="hidePassword.set(!hidePassword())">
                <mat-icon>{{ hidePassword() ? 'visibility_off' : 'visibility' }}</mat-icon>
              </button>
              @if (form.controls.password.hasError('required')) {
                <mat-error>Le mot de passe est requis</mat-error>
              }
              @if (form.controls.password.hasError('minlength')) {
                <mat-error>Minimum 8 caracteres</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>Je suis un(e)</mat-label>
              <mat-select formControlName="role">
                <mat-option value="client">Client (acheteur)</mat-option>
                <mat-option value="eleveur">Eleveur (vendeur)</mat-option>
              </mat-select>
              <mat-icon matPrefix>badge</mat-icon>
            </mat-form-field>

            @if (errorMessage()) {
              <div class="error-banner">
                <mat-icon>error_outline</mat-icon>
                <span>{{ errorMessage() }}</span>
              </div>
            }

            <button mat-raised-button color="primary" type="submit"
                    class="full-width submit-btn"
                    [disabled]="form.invalid || submitting()">
              @if (submitting()) {
                <mat-spinner diameter="24"></mat-spinner>
              } @else {
                Creer mon compte
              }
            </button>
          </form>
        </mat-card-content>

        <mat-card-actions align="end">
          <span class="login-link">
            Deja un compte ?
            <a routerLink="/login">Se connecter</a>
          </span>
        </mat-card-actions>
      </mat-card>
    </div>
  `,
  styles: [`
    .register-container {
      display: flex;
      justify-content: center;
      align-items: center;
      min-height: calc(100vh - 64px - 60px);
      padding: 24px;
      background: linear-gradient(135deg, #e8f5e9 0%, #fff8e1 100%);
    }

    .register-card {
      width: 100%;
      max-width: 480px;
      padding: 24px;
    }

    .full-width {
      width: 100%;
    }

    .submit-btn {
      height: 48px;
      font-size: 1rem;
      margin-top: 8px;
    }

    .error-banner {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 12px;
      margin: 8px 0;
      background: #ffebee;
      border-radius: 4px;
      color: var(--faso-warn);
      font-size: 0.9rem;
    }

    .login-link {
      font-size: 0.9rem;
      color: var(--faso-text-secondary);

      a {
        color: var(--faso-primary);
        font-weight: 500;
      }
    }
  `],
})
export class RegisterComponent {
  private readonly fb = inject(FormBuilder);
  private readonly auth = inject(AuthService);
  private readonly router = inject(Router);
  private readonly snackBar = inject(MatSnackBar);

  readonly hidePassword = signal(true);
  readonly submitting = signal(false);
  readonly errorMessage = signal<string | null>(null);

  readonly form = this.fb.nonNullable.group({
    name: ['', [Validators.required]],
    email: ['', [Validators.required, Validators.email]],
    phone: [''],
    password: ['', [Validators.required, Validators.minLength(8)]],
    role: ['client' as 'client' | 'eleveur', [Validators.required]],
  });

  onSubmit(): void {
    if (this.form.invalid) return;

    this.submitting.set(true);
    this.errorMessage.set(null);

    const values = this.form.getRawValue();

    this.auth
      .register({
        email: values.email,
        password: values.password,
        name: values.name,
        role: values.role,
        phone: values.phone || undefined,
      })
      .subscribe({
        next: () => {
          this.submitting.set(false);
          this.snackBar.open('Compte cree avec succes !', 'Fermer', {
            duration: 3000,
            panelClass: 'snackbar-success',
          });
          this.router.navigate(['/']);
        },
        error: (err) => {
          this.submitting.set(false);
          const message =
            err.status === 409
              ? 'Un compte avec cet email existe deja.'
              : 'Erreur lors de la creation du compte.';
          this.errorMessage.set(message);
        },
      });
  }
}
