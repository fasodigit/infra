import { Component, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink, Router } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, Validators, AbstractControl, ValidationErrors } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatSelectModule } from '@angular/material/select';
import { MatStepperModule } from '@angular/material/stepper';
import { MatRadioModule } from '@angular/material/radio';
import { MatChipsModule } from '@angular/material/chips';
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
    MatCardModule,
    MatFormFieldModule,
    MatInputModule,
    MatButtonModule,
    MatIconModule,
    MatSelectModule,
    MatStepperModule,
    MatRadioModule,
    MatChipsModule,
    MatProgressSpinnerModule,
    TranslateModule,
  ],
  template: `
    <div class="register-page">
      <mat-card class="register-card">
        <mat-card-header>
          <mat-card-title class="register-title">
            <span class="brand">Poulets BF</span>
            <span class="subtitle">{{ 'auth.register' | translate }}</span>
          </mat-card-title>
        </mat-card-header>

        <mat-card-content>
          @if (errorMessage()) {
            <div class="error-banner">
              <mat-icon>error_outline</mat-icon>
              <span>{{ errorMessage() | translate }}</span>
            </div>
          }

          <mat-stepper [linear]="true" #stepper>
            <!-- Step 1: Account info -->
            <mat-step [stepControl]="accountForm" [label]="'auth.step_account' | translate">
              <form [formGroup]="accountForm" class="step-form">
                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'auth.name' | translate }}</mat-label>
                  <input matInput formControlName="nom" autocomplete="name">
                  <mat-icon matPrefix>person</mat-icon>
                  @if (accountForm.get('nom')?.hasError('required') && accountForm.get('nom')?.touched) {
                    <mat-error>{{ 'common.required_field' | translate }}</mat-error>
                  }
                </mat-form-field>

                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'auth.email' | translate }}</mat-label>
                  <input matInput type="email" formControlName="email" autocomplete="email">
                  <mat-icon matPrefix>email</mat-icon>
                  @if (accountForm.get('email')?.hasError('required') && accountForm.get('email')?.touched) {
                    <mat-error>{{ 'common.required_field' | translate }}</mat-error>
                  }
                  @if (accountForm.get('email')?.hasError('email') && accountForm.get('email')?.touched) {
                    <mat-error>{{ 'common.invalid_email' | translate }}</mat-error>
                  }
                </mat-form-field>

                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'auth.phone' | translate }}</mat-label>
                  <input matInput formControlName="phone" autocomplete="tel">
                  <mat-icon matPrefix>phone</mat-icon>
                </mat-form-field>

                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'auth.password' | translate }}</mat-label>
                  <input matInput type="password" formControlName="password" autocomplete="new-password">
                  <mat-icon matPrefix>lock</mat-icon>
                  @if (accountForm.get('password')?.hasError('required') && accountForm.get('password')?.touched) {
                    <mat-error>{{ 'common.required_field' | translate }}</mat-error>
                  }
                  @if (accountForm.get('password')?.hasError('minlength') && accountForm.get('password')?.touched) {
                    <mat-error>{{ 'common.password_min_length' | translate }}</mat-error>
                  }
                </mat-form-field>

                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'auth.confirm_password' | translate }}</mat-label>
                  <input matInput type="password" formControlName="confirmPassword" autocomplete="new-password">
                  <mat-icon matPrefix>lock_outline</mat-icon>
                  @if (accountForm.get('confirmPassword')?.hasError('passwordMismatch') && accountForm.get('confirmPassword')?.touched) {
                    <mat-error>{{ 'common.passwords_mismatch' | translate }}</mat-error>
                  }
                </mat-form-field>

                <div class="step-actions">
                  <span></span>
                  <button mat-raised-button color="primary" matStepperNext
                          [disabled]="accountForm.invalid">
                    {{ 'auth.next' | translate }}
                  </button>
                </div>
              </form>
            </mat-step>

            <!-- Step 2: Role selection -->
            <mat-step [stepControl]="roleForm" [label]="'auth.step_role' | translate">
              <form [formGroup]="roleForm" class="step-form">
                <p class="step-instruction">{{ 'auth.role_selection' | translate }}</p>
                <mat-radio-group formControlName="role" class="role-group">
                  <div class="role-option" (click)="roleForm.get('role')?.setValue('eleveur')">
                    <mat-radio-button value="eleveur">
                      <div class="role-content">
                        <mat-icon>agriculture</mat-icon>
                        <div>
                          <strong>{{ 'auth.role_eleveur' | translate }}</strong>
                          <p>{{ 'auth.role_eleveur_desc' | translate }}</p>
                        </div>
                      </div>
                    </mat-radio-button>
                  </div>
                  <div class="role-option" (click)="roleForm.get('role')?.setValue('client')">
                    <mat-radio-button value="client">
                      <div class="role-content">
                        <mat-icon>shopping_bag</mat-icon>
                        <div>
                          <strong>{{ 'auth.role_client' | translate }}</strong>
                          <p>{{ 'auth.role_client_desc' | translate }}</p>
                        </div>
                      </div>
                    </mat-radio-button>
                  </div>
                  <div class="role-option" (click)="roleForm.get('role')?.setValue('producteur_aliment')">
                    <mat-radio-button value="producteur_aliment">
                      <div class="role-content">
                        <mat-icon>factory</mat-icon>
                        <div>
                          <strong>{{ 'auth.role_producteur' | translate }}</strong>
                          <p>{{ 'auth.role_producteur_desc' | translate }}</p>
                        </div>
                      </div>
                    </mat-radio-button>
                  </div>
                </mat-radio-group>

                <div class="step-actions">
                  <button mat-button matStepperPrevious>
                    {{ 'auth.previous' | translate }}
                  </button>
                  <button mat-raised-button color="primary" matStepperNext
                          [disabled]="roleForm.invalid">
                    {{ 'auth.next' | translate }}
                  </button>
                </div>
              </form>
            </mat-step>

            <!-- Step 3: Role-specific details -->
            <mat-step [label]="'auth.step_details' | translate" [optional]="true">
              <form [formGroup]="detailsForm" class="step-form">
                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'auth.location' | translate }}</mat-label>
                  <input matInput formControlName="localisation">
                  <mat-icon matPrefix>location_on</mat-icon>
                </mat-form-field>

                @if (selectedRole() === 'eleveur') {
                  <mat-form-field appearance="outline" class="full-width">
                    <mat-label>{{ 'auth.capacity' | translate }}</mat-label>
                    <input matInput type="number" formControlName="capacite">
                    <mat-icon matPrefix>inventory_2</mat-icon>
                  </mat-form-field>
                }

                @if (selectedRole() === 'client') {
                  <mat-form-field appearance="outline" class="full-width">
                    <mat-label>{{ 'auth.client_type' | translate }}</mat-label>
                    <mat-select formControlName="clientType">
                      <mat-option value="restaurant">{{ 'auth.client_restaurant' | translate }}</mat-option>
                      <mat-option value="menage">{{ 'auth.client_household' | translate }}</mat-option>
                      <mat-option value="revendeur">{{ 'auth.client_reseller' | translate }}</mat-option>
                      <mat-option value="evenement">{{ 'auth.client_event' | translate }}</mat-option>
                    </mat-select>
                  </mat-form-field>
                }

                @if (selectedRole() === 'producteur_aliment') {
                  <mat-form-field appearance="outline" class="full-width">
                    <mat-label>{{ 'auth.distribution_zone' | translate }}</mat-label>
                    <input matInput formControlName="zoneDistribution">
                    <mat-icon matPrefix>map</mat-icon>
                  </mat-form-field>
                }

                <div class="step-actions">
                  <button mat-button matStepperPrevious>
                    {{ 'auth.previous' | translate }}
                  </button>
                  <button mat-raised-button color="primary" matStepperNext>
                    {{ 'auth.next' | translate }}
                  </button>
                </div>
              </form>
            </mat-step>

            <!-- Step 4: Groupement (optional) -->
            <mat-step [label]="'auth.step_groupement' | translate" [optional]="true">
              <form [formGroup]="groupementForm" class="step-form">
                <p class="step-instruction">{{ 'auth.groupement_optional' | translate }}</p>

                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'auth.groupement_name' | translate }}</mat-label>
                  <input matInput formControlName="groupementNom">
                  <mat-icon matPrefix>groups</mat-icon>
                </mat-form-field>

                <div class="step-actions">
                  <button mat-button matStepperPrevious>
                    {{ 'auth.previous' | translate }}
                  </button>
                  <button
                    mat-raised-button
                    color="primary"
                    [disabled]="loading()"
                    (click)="onSubmit()"
                  >
                    @if (loading()) {
                      <mat-spinner diameter="20"></mat-spinner>
                    } @else {
                      {{ 'auth.finish' | translate }}
                    }
                  </button>
                </div>
              </form>
            </mat-step>
          </mat-stepper>

          <div class="login-link">
            {{ 'auth.have_account' | translate }}
            <a routerLink="/auth/login">{{ 'auth.sign_in' | translate }}</a>
          </div>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .register-page {
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 100vh;
      background: linear-gradient(135deg, #1b5e20 0%, #2e7d32 50%, #43a047 100%);
      padding: 24px;
    }

    .register-card {
      width: 100%;
      max-width: 640px;
      padding: 24px;
    }

    .register-title {
      text-align: center;
      width: 100%;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 8px;
      margin-bottom: 16px;
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

    .step-form {
      padding: 16px 0;
    }

    .step-instruction {
      color: #666;
      margin-bottom: 16px;
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

    .role-group {
      display: flex;
      flex-direction: column;
      gap: 12px;
    }

    .role-option {
      border: 1px solid #e0e0e0;
      border-radius: 8px;
      padding: 16px;
      cursor: pointer;
      transition: border-color 0.2s;
    }

    .role-option:hover {
      border-color: #2e7d32;
    }

    .role-content {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-left: 8px;
    }

    .role-content mat-icon {
      font-size: 32px;
      width: 32px;
      height: 32px;
      color: #2e7d32;
    }

    .role-content p {
      margin: 4px 0 0;
      font-size: 0.85rem;
      color: #666;
      font-weight: 400;
    }

    .step-actions {
      display: flex;
      justify-content: space-between;
      margin-top: 24px;
    }

    .login-link {
      margin-top: 24px;
      text-align: center;
      font-size: 0.9rem;
      color: #666;
    }

    .login-link a {
      color: #2e7d32;
      font-weight: 500;
      text-decoration: none;
    }

    .login-link a:hover {
      text-decoration: underline;
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

  // Step 1: Account
  readonly accountForm = this.fb.nonNullable.group({
    nom: ['', Validators.required],
    email: ['', [Validators.required, Validators.email]],
    phone: [''],
    password: ['', [Validators.required, Validators.minLength(8)]],
    confirmPassword: ['', [Validators.required]],
  }, { validators: [this.passwordMatchValidator] });

  // Step 2: Role
  readonly roleForm = this.fb.nonNullable.group({
    role: ['client' as Role, Validators.required],
  });

  // Step 3: Details
  readonly detailsForm = this.fb.group({
    localisation: [''],
    capacite: [null as number | null],
    clientType: [''],
    zoneDistribution: [''],
  });

  // Step 4: Groupement
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
