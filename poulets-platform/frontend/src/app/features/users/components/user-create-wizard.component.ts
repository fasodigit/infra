// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, inject, signal, computed } from '@angular/core';
import { CommonModule } from '@angular/common';
import { Router, RouterLink } from '@angular/router';
import { FormsModule, ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatStepperModule } from '@angular/material/stepper';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatSnackBar } from '@angular/material/snack-bar';

import { PROJECT_CONFIG, UserRole, AdminLevel } from '@core/config/project-config.token';
import { UsersService } from '../services/users.service';

const REGIONS_BF = [
  'Boucle du Mouhoun', 'Cascades', 'Centre', 'Centre-Est', 'Centre-Nord',
  'Centre-Ouest', 'Centre-Sud', 'Est', 'Hauts-Bassins', 'Nord',
  'Plateau-Central', 'Sahel', 'Sud-Ouest',
];

@Component({
  selector: 'app-user-create-wizard',
  standalone: true,
  imports: [
    CommonModule, RouterLink, FormsModule, ReactiveFormsModule,
    MatIconModule, MatButtonModule, MatStepperModule, MatFormFieldModule,
    MatInputModule, MatSelectModule, MatCheckboxModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <a mat-button routerLink="/admin/users" class="back">
        <mat-icon>arrow_back</mat-icon> Retour à la liste
      </a>
      <header>
        <h1>Créer un utilisateur</h1>
        <p>Trois étapes · invitation envoyée par email via ORY Kratos</p>
      </header>

      @if (success()) {
        <div class="success">
          <mat-icon>check_circle</mat-icon>
          <div>
            <h2>Utilisateur créé avec succès</h2>
            <p>Un email d'invitation a été envoyé à <strong>{{ baseForm.value.email }}</strong>.</p>
            <p class="link">Lien magique (dev only) : <code>{{ invitationLink() }}</code></p>
          </div>
          <div class="success-cta">
            <a mat-raised-button color="primary" routerLink="/admin/users">Voir tous les utilisateurs</a>
            <button mat-stroked-button type="button" (click)="reset()">Créer un autre utilisateur</button>
          </div>
        </div>
      } @else {
        <mat-stepper linear orientation="horizontal" #stepper>
          <!-- Step 1: Role choice -->
          <mat-step [editable]="true">
            <ng-template matStepLabel>Type de compte</ng-template>
            <div class="step">
              <p class="step-hint">Chaque rôle donne accès à un espace et des capacités différentes.</p>
              <div class="role-grid">
                @for (opt of config.availableRoles; track opt.value) {
                  <button
                    type="button"
                    class="role-card"
                    [class.selected]="selectedRole() === opt.value"
                    (click)="pickRole(opt.value); stepper.next()"
                  >
                    <span class="role-icon"><mat-icon>{{ opt.icon }}</mat-icon></span>
                    <strong>{{ opt.label }}</strong>
                    <p>{{ opt.description }}</p>
                  </button>
                }
              </div>
            </div>
          </mat-step>

          <!-- Step 2: Base info -->
          <mat-step [stepControl]="baseForm">
            <ng-template matStepLabel>Informations générales</ng-template>
            <form [formGroup]="baseForm" class="step form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Prénom</mat-label>
                <input matInput formControlName="firstName" required>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Nom</mat-label>
                <input matInput formControlName="lastName" required>
              </mat-form-field>
              <mat-form-field appearance="outline" class="full">
                <mat-label>Email</mat-label>
                <input matInput type="email" formControlName="email" required>
                <mat-hint>Recevra le lien d'invitation Kratos.</mat-hint>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Téléphone (+226)</mat-label>
                <input matInput formControlName="phone" placeholder="70 12 34 56">
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Région</mat-label>
                <mat-select formControlName="region">
                  @for (r of regions; track r) { <mat-option [value]="r">{{ r }}</mat-option> }
                </mat-select>
              </mat-form-field>

              <div class="actions full">
                <button mat-button type="button" matStepperPrevious>Retour</button>
                <button mat-raised-button color="primary" type="button" matStepperNext
                        [disabled]="baseForm.invalid">Continuer</button>
              </div>
            </form>
          </mat-step>

          <!-- Step 3: Role-specific -->
          <mat-step [stepControl]="roleForm">
            <ng-template matStepLabel>Configuration {{ roleLabel() }}</ng-template>
            <form [formGroup]="roleForm" class="step form-grid">
              @switch (selectedRole()) {
                @case ('ELEVEUR') {
                  <mat-form-field appearance="outline">
                    <mat-label>Fiche vétérinaire (ID)</mat-label>
                    <input matInput formControlName="ficheVeterinaire" placeholder="VET-2026-001">
                  </mat-form-field>
                  <mat-form-field appearance="outline">
                    <mat-label>Groupement / Coopérative</mat-label>
                    <input matInput formControlName="groupement" placeholder="(facultatif)">
                  </mat-form-field>
                  <mat-checkbox formControlName="halalInitial" class="full">
                    Éleveur déjà certifié halal
                  </mat-checkbox>
                  <mat-checkbox formControlName="bioInitial" class="full">
                    Production biologique
                  </mat-checkbox>
                }
                @case ('CLIENT') {
                  <mat-form-field appearance="outline">
                    <mat-label>Type de client</mat-label>
                    <mat-select formControlName="clientType">
                      <mat-option value="PARTICULIER">Particulier</mat-option>
                      <mat-option value="RESTAURANT">Restaurant / hôtel</mat-option>
                      <mat-option value="REVENDEUR">Revendeur / grossiste</mat-option>
                    </mat-select>
                  </mat-form-field>
                  <mat-form-field appearance="outline" class="full">
                    <mat-label>Adresse principale</mat-label>
                    <input matInput formControlName="address" placeholder="Ouaga 2000, secteur 15…">
                  </mat-form-field>
                }
                @case ('PRODUCTEUR') {
                  <mat-form-field appearance="outline">
                    <mat-label>Produits</mat-label>
                    <mat-select formControlName="productType" multiple>
                      <mat-option value="ALIMENT">Aliments (pré-starter, starter, finisher)</mat-option>
                      <mat-option value="POUSSIN">Poussins</mat-option>
                      <mat-option value="PHARMA">Produits pharmaceutiques</mat-option>
                    </mat-select>
                  </mat-form-field>
                  <mat-form-field appearance="outline">
                    <mat-label>Zone de livraison</mat-label>
                    <mat-select formControlName="deliveryZone" multiple>
                      @for (r of regions; track r) { <mat-option [value]="r">{{ r }}</mat-option> }
                    </mat-select>
                  </mat-form-field>
                }
                @case ('ADMIN') {
                  <mat-form-field appearance="outline" class="full">
                    <mat-label>Niveau d'administration</mat-label>
                    <mat-select formControlName="adminLevel">
                      <mat-option value="ADMIN_SUPPORT">Admin Support (lecture seule + acquit alertes)</mat-option>
                      <mat-option value="ADMIN_MODERATION">Admin Modération (users + content)</mat-option>
                      <mat-option value="SUPER_ADMIN">Super Admin (full access, config plateforme)</mat-option>
                    </mat-select>
                  </mat-form-field>
                }
              }

              <mat-checkbox formControlName="sendInvitation" class="full">
                Envoyer un email d'invitation avec lien magique (recommandé)
              </mat-checkbox>

              <div class="actions full">
                <button mat-button type="button" matStepperPrevious>Retour</button>
                <button mat-raised-button color="primary" type="button"
                        [disabled]="baseForm.invalid || roleForm.invalid || submitting()"
                        (click)="submit()">
                  @if (submitting()) { Création… } @else { Créer l'utilisateur }
                </button>
              </div>
            </form>
          </mat-step>
        </mat-stepper>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    .back { margin-left: calc(var(--faso-space-4) * -1); color: var(--faso-text-muted); }
    header { margin: var(--faso-space-2) 0 var(--faso-space-6); }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .step { padding: var(--faso-space-4) 0; }
    .step-hint { color: var(--faso-text-muted); margin: 0 0 var(--faso-space-4); }

    .role-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: var(--faso-space-3);
    }
    .role-card {
      text-align: left;
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      cursor: pointer;
      transition: border-color var(--faso-duration-fast) var(--faso-ease-standard),
                  transform var(--faso-duration-fast) var(--faso-ease-standard),
                  box-shadow var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .role-card:hover {
      border-color: var(--faso-primary-300);
      transform: translateY(-2px);
      box-shadow: var(--faso-shadow-md);
    }
    .role-card.selected {
      border-color: var(--faso-primary-600);
      background: var(--faso-primary-50);
      box-shadow: var(--faso-shadow-md);
    }
    .role-icon {
      display: inline-flex;
      width: 48px; height: 48px;
      border-radius: 12px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      align-items: center;
      justify-content: center;
      margin-bottom: var(--faso-space-3);
    }
    .role-card.selected .role-icon {
      background: var(--faso-primary-600);
      color: var(--faso-text-inverse);
    }
    .role-card strong {
      display: block;
      font-size: var(--faso-text-lg);
      margin-bottom: 4px;
    }
    .role-card p { margin: 0; color: var(--faso-text-muted); font-size: var(--faso-text-sm); }

    .form-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--faso-space-3);
    }
    .form-grid .full { grid-column: 1 / -1; }
    .actions {
      display: flex;
      justify-content: flex-end;
      gap: var(--faso-space-2);
      margin-top: var(--faso-space-3);
    }
    @media (max-width: 639px) { .form-grid { grid-template-columns: 1fr; } }

    .success {
      display: flex;
      align-items: center;
      gap: var(--faso-space-4);
      padding: var(--faso-space-6);
      background: var(--faso-success-bg);
      border: 1px solid var(--faso-success);
      border-radius: var(--faso-radius-xl);
      flex-wrap: wrap;
    }
    .success mat-icon {
      font-size: 56px; width: 56px; height: 56px;
      color: var(--faso-success);
      flex-shrink: 0;
    }
    .success h2 { margin: 0 0 var(--faso-space-2); color: var(--faso-primary-800); }
    .success p { margin: 0 0 var(--faso-space-1); color: var(--faso-text-muted); }
    .success .link code {
      display: inline-block;
      background: #FFFFFF;
      padding: 4px 8px;
      border-radius: var(--faso-radius-sm);
      font-family: var(--faso-font-mono);
      font-size: var(--faso-text-xs);
      word-break: break-all;
    }
    .success-cta {
      display: flex;
      gap: var(--faso-space-2);
      flex-basis: 100%;
      padding-top: var(--faso-space-2);
      border-top: 1px solid var(--faso-border);
    }
  `],
})
export class UserCreateWizardComponent {
  private readonly fb = inject(FormBuilder);
  private readonly router = inject(Router);
  private readonly snack = inject(MatSnackBar);
  private readonly users = inject(UsersService);
  readonly config = inject(PROJECT_CONFIG);

  readonly regions = REGIONS_BF;

  readonly selectedRole = signal<UserRole | null>(null);
  readonly submitting = signal(false);
  readonly success = signal(false);
  readonly invitationLink = signal('');

  readonly roleLabel = computed(() => {
    const r = this.selectedRole();
    return this.config.availableRoles.find((o) => o.value === r)?.label ?? '';
  });

  readonly baseForm = this.fb.group({
    firstName: ['', [Validators.required, Validators.minLength(2)]],
    lastName:  ['', [Validators.required, Validators.minLength(2)]],
    email:     ['', [Validators.required, Validators.email]],
    phone:     ['', [Validators.pattern(/^[\d\s+]{8,}$/)]],
    region:    [''],
  });

  readonly roleForm = this.fb.group({
    // ELEVEUR
    ficheVeterinaire: [''],
    groupement: [''],
    halalInitial: [false],
    bioInitial: [false],
    // CLIENT
    clientType: [''],
    address: [''],
    // PRODUCTEUR
    productType: [[] as string[]],
    deliveryZone: [[] as string[]],
    // ADMIN
    adminLevel: ['' as AdminLevel | ''],
    // Common
    sendInvitation: [true],
  });

  pickRole(r: UserRole): void { this.selectedRole.set(r); }

  submit(): void {
    const role = this.selectedRole();
    if (!role) return;
    this.submitting.set(true);

    const base = this.baseForm.value;
    const rf = this.roleForm.value;
    const roleMeta = this.buildRoleMeta(role, rf);

    this.users.create({
      role,
      email: base.email ?? '',
      firstName: base.firstName ?? '',
      lastName: base.lastName ?? '',
      phone: base.phone || undefined,
      region: base.region || undefined,
      roleMeta,
      sendInvitation: !!rf.sendInvitation,
    }).subscribe({
      next: (res) => {
        this.invitationLink.set(res.invitationLink);
        this.success.set(true);
        this.submitting.set(false);
        this.snack.open('Utilisateur créé, invitation envoyée', 'OK', { duration: 3500 });
      },
      error: (err) => {
        this.submitting.set(false);
        this.snack.open('Erreur : ' + (err?.message ?? 'création impossible'), 'OK', { duration: 4500 });
      },
    });
  }

  reset(): void {
    this.baseForm.reset();
    this.roleForm.reset({ sendInvitation: true });
    this.selectedRole.set(null);
    this.success.set(false);
    this.invitationLink.set('');
  }

  private buildRoleMeta(role: UserRole, rf: any): Record<string, unknown> {
    switch (role) {
      case 'ELEVEUR':
        return {
          ficheVeterinaire: rf.ficheVeterinaire || null,
          groupement: rf.groupement || null,
          halalInitial: !!rf.halalInitial,
          bioInitial: !!rf.bioInitial,
        };
      case 'CLIENT':
        return { clientType: rf.clientType || 'PARTICULIER', address: rf.address || null };
      case 'PRODUCTEUR':
        return { productType: rf.productType ?? [], deliveryZone: rf.deliveryZone ?? [] };
      case 'ADMIN':
        return { adminLevel: rf.adminLevel || 'ADMIN_SUPPORT' };
    }
  }
}
