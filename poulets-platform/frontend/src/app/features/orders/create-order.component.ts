import { Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { Router, RouterLink } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, FormGroup, Validators } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatRadioModule } from '@angular/material/radio';
import { MatStepperModule } from '@angular/material/stepper';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';
import { FcfaCurrencyPipe } from '../../shared/pipes/currency.pipe';
import { Race } from '../../shared/models/poulet.model';

@Component({
  selector: 'app-create-order',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    ReactiveFormsModule,
    MatCardModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    MatButtonModule,
    MatIconModule,
    MatDatepickerModule,
    MatRadioModule,
    MatStepperModule,
    MatDividerModule,
    TranslateModule,
    FcfaCurrencyPipe,
  ],
  template: `
    <div class="create-order-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'orders.create.title' | translate }}</h1>
      </div>

      <mat-stepper linear #stepper>
        <!-- Step 1: Product Selection -->
        <mat-step [stepControl]="productForm" label="{{ 'orders.create.step_product' | translate }}">
          <mat-card>
            <mat-card-content>
              <form [formGroup]="productForm" class="form-grid">
                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'orders.create.race' | translate }}</mat-label>
                  <mat-select formControlName="race">
                    @for (race of races; track race) {
                      <mat-option [value]="race">{{ race }}</mat-option>
                    }
                  </mat-select>
                  @if (productForm.get('race')?.hasError('required')) {
                    <mat-error>{{ 'orders.create.race_required' | translate }}</mat-error>
                  }
                </mat-form-field>

                <mat-form-field appearance="outline">
                  <mat-label>{{ 'orders.create.quantity' | translate }}</mat-label>
                  <input matInput type="number" formControlName="quantite" min="1">
                  @if (productForm.get('quantite')?.hasError('required')) {
                    <mat-error>{{ 'orders.create.quantity_required' | translate }}</mat-error>
                  }
                  @if (productForm.get('quantite')?.hasError('min')) {
                    <mat-error>{{ 'orders.create.quantity_min' | translate }}</mat-error>
                  }
                </mat-form-field>

                <mat-form-field appearance="outline">
                  <mat-label>{{ 'orders.create.price_unit' | translate }} (FCFA)</mat-label>
                  <input matInput type="number" formControlName="prixUnitaire" min="0">
                </mat-form-field>

                <div class="total-preview">
                  <span class="total-label">{{ 'orders.create.total' | translate }}</span>
                  <span class="total-value">{{ calculatedTotal() | fcfa }}</span>
                </div>
              </form>

              <div class="step-actions">
                <button mat-raised-button color="primary" matStepperNext
                        [disabled]="productForm.invalid">
                  {{ 'common.next' | translate }}
                </button>
              </div>
            </mat-card-content>
          </mat-card>
        </mat-step>

        <!-- Step 2: Delivery -->
        <mat-step [stepControl]="deliveryForm" label="{{ 'orders.create.step_delivery' | translate }}">
          <mat-card>
            <mat-card-content>
              <form [formGroup]="deliveryForm" class="form-grid">
                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'orders.create.delivery_date' | translate }}</mat-label>
                  <input matInput [matDatepicker]="picker" formControlName="dateLivraison">
                  <mat-datepicker-toggle matIconSuffix [for]="picker"></mat-datepicker-toggle>
                  <mat-datepicker #picker></mat-datepicker>
                  @if (deliveryForm.get('dateLivraison')?.hasError('required')) {
                    <mat-error>{{ 'orders.create.delivery_date_required' | translate }}</mat-error>
                  }
                </mat-form-field>

                <div class="radio-group full-width">
                  <label class="radio-label">{{ 'orders.create.delivery_mode' | translate }}</label>
                  <mat-radio-group formControlName="modeLivraison">
                    <mat-radio-button value="self">
                      {{ 'orders.create.mode_self' | translate }}
                    </mat-radio-button>
                    <mat-radio-button value="livreur_tiers">
                      {{ 'orders.create.mode_third_party' | translate }}
                    </mat-radio-button>
                  </mat-radio-group>
                </div>

                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'orders.create.delivery_address' | translate }}</mat-label>
                  <input matInput formControlName="adresseLivraison">
                  @if (deliveryForm.get('adresseLivraison')?.hasError('required')) {
                    <mat-error>{{ 'orders.create.address_required' | translate }}</mat-error>
                  }
                </mat-form-field>

                <mat-form-field appearance="outline">
                  <mat-label>{{ 'orders.create.phone' | translate }}</mat-label>
                  <input matInput formControlName="telephone" placeholder="+226 XX XX XX XX">
                  @if (deliveryForm.get('telephone')?.hasError('required')) {
                    <mat-error>{{ 'orders.create.phone_required' | translate }}</mat-error>
                  }
                </mat-form-field>
              </form>

              <div class="step-actions">
                <button mat-button matStepperPrevious>{{ 'common.back' | translate }}</button>
                <button mat-raised-button color="primary" matStepperNext
                        [disabled]="deliveryForm.invalid">
                  {{ 'common.next' | translate }}
                </button>
              </div>
            </mat-card-content>
          </mat-card>
        </mat-step>

        <!-- Step 3: Payment -->
        <mat-step [stepControl]="paymentForm" label="{{ 'orders.create.step_payment' | translate }}">
          <mat-card>
            <mat-card-content>
              <form [formGroup]="paymentForm" class="form-grid">
                <div class="radio-group full-width">
                  <label class="radio-label">{{ 'orders.create.payment_method' | translate }}</label>
                  <mat-radio-group formControlName="modePaiement">
                    <mat-radio-button value="orange_money">
                      <span class="payment-option">
                        <mat-icon>phone_android</mat-icon>
                        Orange Money
                      </span>
                    </mat-radio-button>
                    <mat-radio-button value="moov_money">
                      <span class="payment-option">
                        <mat-icon>phone_android</mat-icon>
                        Moov Money
                      </span>
                    </mat-radio-button>
                    <mat-radio-button value="cash">
                      <span class="payment-option">
                        <mat-icon>payments</mat-icon>
                        {{ 'orders.create.cash' | translate }}
                      </span>
                    </mat-radio-button>
                  </mat-radio-group>
                </div>

                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'orders.create.notes' | translate }}</mat-label>
                  <textarea matInput formControlName="notes" rows="3"></textarea>
                </mat-form-field>
              </form>

              <!-- Order Summary -->
              <mat-divider></mat-divider>
              <div class="order-summary">
                <h3>{{ 'orders.create.summary' | translate }}</h3>
                <div class="summary-row">
                  <span>{{ productForm.get('race')?.value }}</span>
                  <span>x {{ productForm.get('quantite')?.value }}</span>
                </div>
                <div class="summary-row total">
                  <span>{{ 'orders.create.total' | translate }}</span>
                  <span>{{ calculatedTotal() | fcfa }}</span>
                </div>
              </div>

              <div class="step-actions">
                <button mat-button matStepperPrevious>{{ 'common.back' | translate }}</button>
                <button mat-raised-button color="primary" (click)="submitOrder()"
                        [disabled]="paymentForm.invalid || submitting()">
                  <mat-icon>check</mat-icon>
                  {{ 'orders.create.confirm' | translate }}
                </button>
              </div>
            </mat-card-content>
          </mat-card>
        </mat-step>
      </mat-stepper>
    </div>
  `,
  styles: [`
    .create-order-container {
      padding: 24px;
      max-width: 800px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .form-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
      padding: 16px 0;
    }

    .full-width { grid-column: 1 / -1; }

    .radio-group {
      display: flex;
      flex-direction: column;
      gap: 8px;

      .radio-label {
        font-size: 0.875rem;
        color: rgba(0, 0, 0, 0.6);
      }

      mat-radio-group {
        display: flex;
        flex-direction: column;
        gap: 8px;
      }
    }

    .payment-option {
      display: inline-flex;
      align-items: center;
      gap: 8px;
    }

    .total-preview {
      grid-column: 1 / -1;
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 16px;
      background: #f5f5f5;
      border-radius: 8px;

      .total-label { font-size: 1rem; color: #666; }
      .total-value { font-size: 1.4rem; font-weight: 700; color: var(--faso-primary-dark, #1b5e20); }
    }

    .order-summary {
      padding: 16px 0;

      h3 { margin: 0 0 12px; }

      .summary-row {
        display: flex;
        justify-content: space-between;
        padding: 8px 0;

        &.total {
          font-weight: 700;
          font-size: 1.1rem;
          border-top: 1px solid #e0e0e0;
          margin-top: 8px;
          padding-top: 12px;
        }
      }
    }

    .step-actions {
      display: flex;
      justify-content: flex-end;
      gap: 12px;
      padding-top: 16px;
    }
  `],
})
export class CreateOrderComponent {
  private readonly fb = new FormBuilder();
  readonly submitting = signal(false);

  readonly races = Object.values(Race);

  readonly productForm: FormGroup = this.fb.nonNullable.group({
    race: ['', Validators.required],
    quantite: [1, [Validators.required, Validators.min(1)]],
    prixUnitaire: [3500, [Validators.required, Validators.min(0)]],
  });

  readonly deliveryForm: FormGroup = this.fb.nonNullable.group({
    dateLivraison: ['', Validators.required],
    modeLivraison: ['self', Validators.required],
    adresseLivraison: ['', Validators.required],
    telephone: ['', Validators.required],
  });

  readonly paymentForm: FormGroup = this.fb.nonNullable.group({
    modePaiement: ['orange_money', Validators.required],
    notes: [''],
  });

  calculatedTotal(): number {
    const qty = this.productForm.get('quantite')?.value || 0;
    const price = this.productForm.get('prixUnitaire')?.value || 0;
    return qty * price;
  }

  submitOrder(): void {
    if (this.productForm.invalid || this.deliveryForm.invalid || this.paymentForm.invalid) return;
    this.submitting.set(true);
    // TODO: Call API to create order
    console.log('Order submitted:', {
      ...this.productForm.value,
      ...this.deliveryForm.value,
      ...this.paymentForm.value,
      prixTotal: this.calculatedTotal(),
    });
  }
}
