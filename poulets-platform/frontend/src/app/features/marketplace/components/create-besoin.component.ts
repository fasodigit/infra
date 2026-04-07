import { Component, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { Router } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, FormGroup, Validators } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatNativeDateModule } from '@angular/material/core';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDividerModule } from '@angular/material/divider';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { TranslateModule } from '@ngx-translate/core';

import { MarketplaceService } from '../services/marketplace.service';
import {
  CreateBesoinInput,
  BesoinFrequency,
  CHICKEN_RACES,
  DAYS_OF_WEEK,
} from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-create-besoin',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    ReactiveFormsModule,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    MatDatepickerModule,
    MatNativeDateModule,
    MatCheckboxModule,
    MatDividerModule,
    MatProgressSpinnerModule,
    MatSnackBarModule,
    TranslateModule,
  ],
  template: `
    <div class="create-besoin-page">
      <div class="page-header">
        <h1>
          <mat-icon>add_circle_outline</mat-icon>
          {{ 'marketplace.besoins.create.title' | translate }}
        </h1>
      </div>

      <mat-card>
        <mat-card-content>
          <form [formGroup]="form" (ngSubmit)="onSubmit()" class="besoin-form">

            <!-- Section: What You Need -->
            <h3>{{ 'marketplace.besoins.create.sectionWhat' | translate }}</h3>
            <mat-divider></mat-divider>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field wide">
                <mat-label>{{ 'marketplace.besoins.create.races' | translate }}</mat-label>
                <mat-select formControlName="races" multiple required>
                  @for (race of races; track race) {
                    <mat-option [value]="race">{{ race }}</mat-option>
                  }
                </mat-select>
                @if (form.get('races')?.hasError('required') && form.get('races')?.touched) {
                  <mat-error>{{ 'marketplace.besoins.create.racesRequired' | translate }}</mat-error>
                }
              </mat-form-field>
            </div>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.besoins.create.quantity' | translate }}</mat-label>
                <input matInput type="number" formControlName="quantity" min="1" required>
                @if (form.get('quantity')?.hasError('required') && form.get('quantity')?.touched) {
                  <mat-error>{{ 'marketplace.besoins.create.quantityRequired' | translate }}</mat-error>
                }
                @if (form.get('quantity')?.hasError('min')) {
                  <mat-error>{{ 'marketplace.besoins.create.quantityMin' | translate }}</mat-error>
                }
              </mat-form-field>

              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.besoins.create.minimumWeight' | translate }} (kg)</mat-label>
                <input matInput type="number" formControlName="minimumWeight" min="0" step="0.1" required>
                @if (form.get('minimumWeight')?.hasError('required') && form.get('minimumWeight')?.touched) {
                  <mat-error>{{ 'marketplace.besoins.create.weightRequired' | translate }}</mat-error>
                }
              </mat-form-field>

              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.besoins.create.deliveryDate' | translate }}</mat-label>
                <input matInput [matDatepicker]="deliveryPicker" formControlName="deliveryDate" required>
                <mat-datepicker-toggle matIconSuffix [for]="deliveryPicker"></mat-datepicker-toggle>
                <mat-datepicker #deliveryPicker></mat-datepicker>
                @if (form.get('deliveryDate')?.hasError('required') && form.get('deliveryDate')?.touched) {
                  <mat-error>{{ 'marketplace.besoins.create.dateRequired' | translate }}</mat-error>
                }
              </mat-form-field>
            </div>

            <!-- Section: Budget & Location -->
            <h3>{{ 'marketplace.besoins.create.sectionBudget' | translate }}</h3>
            <mat-divider></mat-divider>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.besoins.create.maxBudgetPerKg' | translate }} (FCFA)</mat-label>
                <input matInput type="number" formControlName="maxBudgetPerKg" min="0" required>
                @if (form.get('maxBudgetPerKg')?.hasError('required') && form.get('maxBudgetPerKg')?.touched) {
                  <mat-error>{{ 'marketplace.besoins.create.budgetRequired' | translate }}</mat-error>
                }
              </mat-form-field>

              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.besoins.create.location' | translate }}</mat-label>
                <input matInput formControlName="location" required>
                @if (form.get('location')?.hasError('required') && form.get('location')?.touched) {
                  <mat-error>{{ 'marketplace.besoins.create.locationRequired' | translate }}</mat-error>
                }
              </mat-form-field>
            </div>

            <!-- Section: Frequency -->
            <h3>{{ 'marketplace.besoins.create.sectionFrequency' | translate }}</h3>
            <mat-divider></mat-divider>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.besoins.create.frequency' | translate }}</mat-label>
                <mat-select formControlName="frequency" required>
                  @for (freq of frequencies; track freq.value) {
                    <mat-option [value]="freq.value">{{ freq.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
            </div>

            @if (isRecurring()) {
              <div class="recurring-section">
                <div class="form-row">
                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'marketplace.besoins.create.recurringStart' | translate }}</mat-label>
                    <input matInput [matDatepicker]="recurStartPicker" formControlName="recurringStartDate">
                    <mat-datepicker-toggle matIconSuffix [for]="recurStartPicker"></mat-datepicker-toggle>
                    <mat-datepicker #recurStartPicker></mat-datepicker>
                  </mat-form-field>

                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'marketplace.besoins.create.recurringEnd' | translate }}</mat-label>
                    <input matInput [matDatepicker]="recurEndPicker" formControlName="recurringEndDate">
                    <mat-datepicker-toggle matIconSuffix [for]="recurEndPicker"></mat-datepicker-toggle>
                    <mat-datepicker #recurEndPicker></mat-datepicker>
                  </mat-form-field>

                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'marketplace.besoins.create.dayPreference' | translate }}</mat-label>
                    <mat-select formControlName="dayOfWeekPreference">
                      @for (day of daysOfWeek; track day.value) {
                        <mat-option [value]="day.value">{{ day.label }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                </div>
              </div>
            }

            <!-- Section: Requirements -->
            <h3>{{ 'marketplace.besoins.create.sectionRequirements' | translate }}</h3>
            <mat-divider></mat-divider>

            <div class="checkbox-row">
              <mat-checkbox formControlName="halalRequired">
                {{ 'marketplace.besoins.create.halalRequired' | translate }}
              </mat-checkbox>
            </div>

            <div class="checkbox-row">
              <mat-checkbox formControlName="veterinaryCertifiedRequired">
                {{ 'marketplace.besoins.create.vetRequired' | translate }}
              </mat-checkbox>
            </div>

            <mat-form-field appearance="outline" class="form-field full-width">
              <mat-label>{{ 'marketplace.besoins.create.specialNotes' | translate }}</mat-label>
              <textarea matInput formControlName="specialNotes" rows="3"></textarea>
              <mat-hint>{{ 'marketplace.besoins.create.specialNotesHint' | translate }}</mat-hint>
            </mat-form-field>

            <!-- Actions -->
            <div class="form-actions">
              <button mat-button type="button" (click)="cancel()">
                {{ 'common.cancel' | translate }}
              </button>
              <button mat-raised-button color="primary" type="submit"
                [disabled]="form.invalid || submitting()">
                @if (submitting()) {
                  <mat-spinner diameter="20"></mat-spinner>
                } @else {
                  <mat-icon>publish</mat-icon>
                  {{ 'marketplace.besoins.create.submit' | translate }}
                }
              </button>
            </div>

          </form>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .create-besoin-page {
      padding: 24px;
      max-width: 900px;
      margin: 0 auto;
    }

    .page-header h1 {
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .besoin-form h3 {
      margin: 24px 0 8px;
      color: #333;
    }

    .besoin-form h3:first-of-type {
      margin-top: 0;
    }

    mat-divider {
      margin-bottom: 16px;
    }

    .form-row {
      display: flex;
      gap: 16px;
      flex-wrap: wrap;
    }

    .form-field {
      flex: 1 1 200px;
      min-width: 200px;
    }

    .form-field.wide {
      flex: 1 1 100%;
    }

    .form-field.full-width {
      width: 100%;
    }

    .recurring-section {
      padding: 16px;
      background: #f5f5f5;
      border-radius: 8px;
      margin-top: 8px;
    }

    .checkbox-row {
      margin: 12px 0;
    }

    .form-actions {
      display: flex;
      justify-content: flex-end;
      gap: 12px;
      margin-top: 32px;
      padding-top: 16px;
      border-top: 1px solid rgba(0, 0, 0, 0.12);
    }
  `],
})
export class CreateBesoinComponent {
  private readonly marketplace = inject(MarketplaceService);
  private readonly fb = inject(FormBuilder);
  private readonly router = inject(Router);
  private readonly snackBar = inject(MatSnackBar);

  readonly races = CHICKEN_RACES;
  readonly daysOfWeek = DAYS_OF_WEEK;
  readonly submitting = signal(false);

  readonly frequencies: { value: BesoinFrequency; label: string }[] = [
    { value: 'PONCTUEL', label: 'Ponctuel (une seule fois)' },
    { value: 'HEBDOMADAIRE', label: 'Hebdomadaire' },
    { value: 'BI_MENSUEL', label: 'Bi-mensuel' },
    { value: 'MENSUEL', label: 'Mensuel' },
  ];

  readonly form: FormGroup = this.fb.group({
    races: [[], Validators.required],
    quantity: [null, [Validators.required, Validators.min(1)]],
    minimumWeight: [null, [Validators.required, Validators.min(0)]],
    deliveryDate: [null, Validators.required],
    maxBudgetPerKg: [null, [Validators.required, Validators.min(0)]],
    location: ['', Validators.required],
    frequency: ['PONCTUEL', Validators.required],
    recurringStartDate: [null],
    recurringEndDate: [null],
    dayOfWeekPreference: [null],
    halalRequired: [false],
    veterinaryCertifiedRequired: [false],
    specialNotes: [''],
  });

  isRecurring(): boolean {
    return this.form.get('frequency')?.value !== 'PONCTUEL';
  }

  onSubmit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }

    this.submitting.set(true);
    const v = this.form.value;

    const input: CreateBesoinInput = {
      races: v.races,
      quantity: v.quantity,
      minimumWeight: v.minimumWeight,
      deliveryDate: new Date(v.deliveryDate).toISOString(),
      maxBudgetPerKg: v.maxBudgetPerKg,
      location: v.location,
      frequency: v.frequency,
      halalRequired: v.halalRequired,
      veterinaryCertifiedRequired: v.veterinaryCertifiedRequired,
      specialNotes: v.specialNotes || undefined,
    };

    if (this.isRecurring()) {
      if (v.recurringStartDate) {
        input.recurringStartDate = new Date(v.recurringStartDate).toISOString();
      }
      if (v.recurringEndDate) {
        input.recurringEndDate = new Date(v.recurringEndDate).toISOString();
      }
      if (v.dayOfWeekPreference != null) {
        input.dayOfWeekPreference = v.dayOfWeekPreference;
      }
    }

    this.marketplace.createBesoin(input).subscribe({
      next: (besoin) => {
        this.submitting.set(false);
        this.snackBar.open('Besoin publie avec succes', 'OK', { duration: 3000 });
        this.router.navigate(['/marketplace/besoins', besoin.id]);
      },
      error: () => {
        this.submitting.set(false);
        this.snackBar.open('Erreur lors de la publication', 'OK', { duration: 3000 });
      },
    });
  }

  cancel(): void {
    this.router.navigate(['/marketplace/besoins']);
  }
}
