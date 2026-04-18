// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, inject, signal, computed } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { Router, RouterLink } from '@angular/router';
import { FormsModule, ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatStepperModule } from '@angular/material/stepper';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatRadioModule } from '@angular/material/radio';
import { MatCheckboxModule } from '@angular/material/checkbox';

import { PanierService } from '@services/panier.service';
import { EmptyStateComponent } from '@shared/components/empty-state/empty-state.component';
import { TrustBadgeComponent } from '@shared/components/trust-badge/trust-badge.component';

/** Les 13 régions du Burkina Faso */
const REGIONS_BF = [
  'Boucle du Mouhoun', 'Cascades', 'Centre', 'Centre-Est', 'Centre-Nord',
  'Centre-Ouest', 'Centre-Sud', 'Est', 'Hauts-Bassins', 'Nord',
  'Plateau-Central', 'Sahel', 'Sud-Ouest',
];

type PaymentMethod = 'orange_money' | 'moov_money' | 'cash';

@Component({
  selector: 'app-checkout',
  standalone: true,
  imports: [
    CommonModule, RouterLink, DecimalPipe, FormsModule, ReactiveFormsModule,
    MatIconModule, MatButtonModule, MatStepperModule, MatFormFieldModule,
    MatInputModule, MatSelectModule, MatRadioModule, MatCheckboxModule,
    EmptyStateComponent, TrustBadgeComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (panier.items().length === 0 && !submitted()) {
      <div class="page">
        <div class="container">
          <app-empty-state icon="shopping_basket" title="Panier vide">
            <a mat-raised-button color="primary" routerLink="/marketplace/annonces">
              Parcourir les annonces
            </a>
          </app-empty-state>
        </div>
      </div>
    } @else if (submitted()) {
      <section class="success">
        <div class="card">
          <mat-icon class="check">check_circle</mat-icon>
          <h1>Commande confirmée&nbsp;!</h1>
          <p>Numéro&nbsp;: <strong>{{ orderNumber() }}</strong></p>
          <p class="sub">L'éleveur va confirmer votre commande sous peu. Vous recevrez une notification.</p>
          <div class="cta">
            <a mat-raised-button color="primary" [routerLink]="['/orders', orderNumber()]">
              Suivre ma commande
            </a>
            <a mat-button routerLink="/marketplace/annonces">Continuer mes achats</a>
          </div>
        </div>
      </section>
    } @else {
      <section class="page">
        <div class="container">
          <header class="head">
            <h1>Finaliser la commande</h1>
            <p>3 étapes simples · paiement sécurisé</p>
          </header>

          <div class="grid">
            <mat-stepper linear="true" orientation="vertical" #stepper>
              <!-- Étape 1 : Livraison -->
              <mat-step [stepControl]="deliveryForm" label="Livraison">
                <form [formGroup]="deliveryForm" class="step-form">
                  <mat-form-field appearance="outline">
                    <mat-label>Nom complet</mat-label>
                    <input matInput formControlName="name" required>
                  </mat-form-field>

                  <mat-form-field appearance="outline">
                    <mat-label>Téléphone (+226)</mat-label>
                    <input matInput formControlName="phone" placeholder="70 12 34 56" required>
                  </mat-form-field>

                  <mat-form-field appearance="outline">
                    <mat-label>Région</mat-label>
                    <mat-select formControlName="region" required>
                      @for (r of regions; track r) {
                        <mat-option [value]="r">{{ r }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>

                  <mat-form-field appearance="outline">
                    <mat-label>Ville / Quartier</mat-label>
                    <input matInput formControlName="city" required>
                  </mat-form-field>

                  <mat-form-field appearance="outline" class="full">
                    <mat-label>Adresse de livraison</mat-label>
                    <textarea matInput rows="2" formControlName="address" required placeholder="Point de repère, secteur…"></textarea>
                  </mat-form-field>

                  <mat-form-field appearance="outline">
                    <mat-label>Date souhaitée</mat-label>
                    <input matInput type="date" formControlName="deliveryDate" [min]="minDate">
                  </mat-form-field>

                  <div class="actions">
                    <button
                      mat-raised-button
                      color="primary"
                      matStepperNext
                      type="button"
                      [disabled]="deliveryForm.invalid"
                    >
                      Continuer
                    </button>
                  </div>
                </form>
              </mat-step>

              <!-- Étape 2 : Paiement -->
              <mat-step [stepControl]="paymentForm" label="Paiement">
                <form [formGroup]="paymentForm" class="step-form">
                  <mat-radio-group formControlName="method" class="methods">
                    <label class="method">
                      <mat-radio-button value="orange_money"></mat-radio-button>
                      <div>
                        <strong>Orange Money</strong>
                        <span>Paiement sécurisé par SMS</span>
                      </div>
                      <mat-icon>phone_iphone</mat-icon>
                    </label>
                    <label class="method">
                      <mat-radio-button value="moov_money"></mat-radio-button>
                      <div>
                        <strong>Moov Money</strong>
                        <span>Paiement sécurisé par SMS</span>
                      </div>
                      <mat-icon>phone_iphone</mat-icon>
                    </label>
                    <label class="method">
                      <mat-radio-button value="cash"></mat-radio-button>
                      <div>
                        <strong>Espèces à la livraison</strong>
                        <span>Payez au livreur à la réception</span>
                      </div>
                      <mat-icon>payments</mat-icon>
                    </label>
                  </mat-radio-group>

                  @if (paymentForm.value.method === 'orange_money' || paymentForm.value.method === 'moov_money') {
                    <mat-form-field appearance="outline">
                      <mat-label>Numéro de paiement (+226)</mat-label>
                      <input matInput formControlName="phoneNumber" placeholder="70 12 34 56">
                    </mat-form-field>
                  }

                  <div class="actions">
                    <button mat-button matStepperPrevious type="button">Retour</button>
                    <button
                      mat-raised-button
                      color="primary"
                      matStepperNext
                      type="button"
                      [disabled]="paymentForm.invalid"
                    >
                      Continuer
                    </button>
                  </div>
                </form>
              </mat-step>

              <!-- Étape 3 : Confirmation -->
              <mat-step label="Confirmation">
                <div class="step-form">
                  <div class="recap">
                    <h3>Livraison</h3>
                    <p>
                      <strong>{{ deliveryForm.value.name }}</strong><br>
                      {{ deliveryForm.value.address }}<br>
                      {{ deliveryForm.value.city }}, {{ deliveryForm.value.region }}<br>
                      {{ deliveryForm.value.phone }}
                    </p>
                  </div>
                  <div class="recap">
                    <h3>Paiement</h3>
                    <p>{{ methodLabel() }}</p>
                  </div>

                  <label class="terms">
                    <mat-checkbox [(ngModel)]="acceptedCgv" [ngModelOptions]="{standalone: true}"></mat-checkbox>
                    <span>J'accepte les conditions générales et la politique de confidentialité.</span>
                  </label>

                  <div class="actions">
                    <button mat-button matStepperPrevious type="button">Retour</button>
                    <button
                      mat-raised-button
                      color="primary"
                      type="button"
                      (click)="submit()"
                      [disabled]="!acceptedCgv || submitting()"
                    >
                      @if (submitting()) {
                        Envoi…
                      } @else {
                        Confirmer la commande · {{ grandTotal() | number:'1.0-0' }} FCFA
                      }
                    </button>
                  </div>
                </div>
              </mat-step>
            </mat-stepper>

            <aside class="summary">
              <h2>Votre commande</h2>
              <ul>
                @for (item of panier.items(); track item.poulet.id) {
                  <li>
                    <img
                      [src]="item.poulet.photos?.[0] || 'assets/img/placeholder-poulet.svg'"
                      [alt]="item.poulet.race"
                      loading="lazy"
                    >
                    <div>
                      <strong>{{ item.poulet.race }}</strong>
                      <span>× {{ item.quantite }}</span>
                    </div>
                    <span class="price">{{ (item.poulet.prix * item.quantite) | number:'1.0-0' }} FCFA</span>
                  </li>
                }
              </ul>
              <dl>
                <div><dt>Sous-total</dt><dd>{{ panier.total() | number:'1.0-0' }} FCFA</dd></div>
                <div><dt>Livraison</dt><dd>{{ shipping() | number:'1.0-0' }} FCFA</dd></div>
                <div class="total"><dt>Total</dt><dd>{{ grandTotal() | number:'1.0-0' }} FCFA</dd></div>
              </dl>
              <div class="trusts">
                <app-trust-badge kind="halal" />
                <app-trust-badge kind="vet" />
                <app-trust-badge kind="flag" />
              </div>
            </aside>
          </div>
        </div>
      </section>
    }
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .container {
      max-width: 1200px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-12);
    }
    .head { margin-bottom: var(--faso-space-6); }
    .head h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    .head p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .grid {
      display: grid;
      grid-template-columns: 1fr 360px;
      gap: var(--faso-space-6);
    }
    @media (max-width: 899px) { .grid { grid-template-columns: 1fr; } }

    mat-stepper {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
    }

    .step-form {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--faso-space-3);
      padding-top: var(--faso-space-2);
    }
    .step-form .full { grid-column: 1 / -1; }
    .step-form .actions {
      grid-column: 1 / -1;
      display: flex;
      justify-content: flex-end;
      gap: var(--faso-space-2);
      margin-top: var(--faso-space-3);
    }

    .methods {
      grid-column: 1 / -1;
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-2);
    }
    .method {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-3);
      align-items: center;
      padding: var(--faso-space-3) var(--faso-space-4);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-lg);
      cursor: pointer;
      transition: border-color var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .method:hover { border-color: var(--faso-primary-300); }
    .method div { display: flex; flex-direction: column; }
    .method strong { font-size: var(--faso-text-base); }
    .method span { color: var(--faso-text-muted); font-size: var(--faso-text-sm); }
    .method mat-icon { color: var(--faso-text-muted); }

    .recap {
      grid-column: 1 / -1;
      padding: var(--faso-space-4);
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-lg);
    }
    .recap h3 { margin: 0 0 8px; font-size: var(--faso-text-sm); text-transform: uppercase; color: var(--faso-text-muted); letter-spacing: 0.05em; }
    .recap p { margin: 0; line-height: var(--faso-leading-relaxed); }

    .terms {
      grid-column: 1 / -1;
      display: flex;
      align-items: flex-start;
      gap: 8px;
      padding: var(--faso-space-3);
      background: var(--faso-info-bg);
      border-radius: var(--faso-radius-md);
      cursor: pointer;
    }

    .summary {
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      box-shadow: var(--faso-shadow-sm);
      position: sticky;
      top: var(--faso-space-6);
      align-self: flex-start;
    }
    .summary h2 { margin: 0 0 var(--faso-space-4); font-size: var(--faso-text-lg); }

    .summary ul { list-style: none; padding: 0; margin: 0 0 var(--faso-space-4); display: flex; flex-direction: column; gap: var(--faso-space-2); }
    .summary li {
      display: grid;
      grid-template-columns: 48px 1fr auto;
      gap: var(--faso-space-2);
      align-items: center;
    }
    .summary li img {
      width: 48px; height: 48px;
      object-fit: cover;
      border-radius: var(--faso-radius-sm);
    }
    .summary li strong { font-size: var(--faso-text-sm); display: block; }
    .summary li span { color: var(--faso-text-muted); font-size: var(--faso-text-xs); }
    .summary li .price {
      color: var(--faso-primary-700);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-semibold);
    }

    .summary dl {
      margin: 0;
      padding-top: var(--faso-space-3);
      border-top: 1px solid var(--faso-border);
      display: flex;
      flex-direction: column;
      gap: 6px;
    }
    .summary dl div { display: flex; justify-content: space-between; align-items: baseline; }
    .summary dl dt { color: var(--faso-text-muted); font-size: var(--faso-text-sm); }
    .summary dl dd { margin: 0; font-weight: var(--faso-weight-medium); }
    .summary dl .total {
      border-top: 1px solid var(--faso-border);
      padding-top: 10px;
      margin-top: 4px;
    }
    .summary dl .total dd {
      font-size: var(--faso-text-xl);
      font-weight: var(--faso-weight-bold);
      color: var(--faso-primary-700);
    }

    .trusts {
      display: flex;
      gap: 4px;
      flex-wrap: wrap;
      margin-top: var(--faso-space-4);
      padding-top: var(--faso-space-3);
      border-top: 1px solid var(--faso-border);
    }

    .success {
      min-height: 70vh;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: var(--faso-space-8);
    }
    .success .card {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-10);
      text-align: center;
      max-width: 480px;
      box-shadow: var(--faso-shadow-md);
    }
    .success .check {
      font-size: 80px; width: 80px; height: 80px;
      color: var(--faso-success);
      margin-bottom: var(--faso-space-3);
    }
    .success h1 { margin: 0 0 var(--faso-space-3); }
    .success p { margin: 0 0 var(--faso-space-2); }
    .success .sub { color: var(--faso-text-muted); margin-bottom: var(--faso-space-6); }
    .success .cta {
      display: flex;
      gap: var(--faso-space-2);
      justify-content: center;
      flex-wrap: wrap;
    }

    @media (max-width: 639px) {
      .step-form { grid-template-columns: 1fr; }
    }
  `],
})
export class CheckoutComponent {
  private readonly fb = inject(FormBuilder);
  private readonly router = inject(Router);
  readonly panier = inject(PanierService);

  readonly regions = REGIONS_BF;
  readonly minDate = new Date().toISOString().split('T')[0];

  readonly submitting = signal(false);
  readonly submitted = signal(false);
  readonly orderNumber = signal('');
  acceptedCgv = false;

  readonly deliveryForm = this.fb.group({
    name: ['', [Validators.required, Validators.minLength(3)]],
    phone: ['', [Validators.required, Validators.pattern(/^[\d\s+]{8,}$/)]],
    region: ['', Validators.required],
    city: ['', Validators.required],
    address: ['', Validators.required],
    deliveryDate: [''],
  });

  readonly paymentForm = this.fb.group({
    method: ['orange_money' as PaymentMethod, Validators.required],
    phoneNumber: [''],
  });

  readonly shipping = computed(() => this.panier.total() > 50000 ? 0 : 2000);
  readonly grandTotal = computed(() => this.panier.total() + this.shipping());

  methodLabel(): string {
    const m = this.paymentForm.value.method;
    return m === 'orange_money' ? 'Orange Money'
         : m === 'moov_money'   ? 'Moov Money'
         : 'Espèces à la livraison';
  }

  submit(): void {
    this.submitting.set(true);
    // Stub: in real flow, POST to BFF createOrder mutation.
    // Simulate latency then empty cart + show confirmation screen.
    setTimeout(() => {
      const n = 'CMD-' + Math.random().toString(36).slice(2, 8).toUpperCase();
      this.orderNumber.set(n);
      this.submitting.set(false);
      this.submitted.set(true);
      this.panier.vider();
    }, 800);
  }
}
