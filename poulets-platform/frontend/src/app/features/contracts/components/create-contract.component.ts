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
import { MatStepperModule } from '@angular/material/stepper';
import { MatDividerModule } from '@angular/material/divider';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { MatAutocompleteModule } from '@angular/material/autocomplete';
import { MatSliderModule } from '@angular/material/slider';
import { TranslateModule } from '@ngx-translate/core';
import { debounceTime, Subject } from 'rxjs';

import { ContractsService, PartnerSearchResult } from '../services/contracts.service';
import {
  CreateContractInput,
  ContractFrequency,
  ContractDuration,
  ContractPriceType,
  computeEndDate,
} from '../../../shared/models/contract.models';
import { CHICKEN_RACES, DAYS_OF_WEEK } from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-create-contract',
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
    MatStepperModule,
    MatDividerModule,
    MatProgressSpinnerModule,
    MatSnackBarModule,
    MatAutocompleteModule,
    MatSliderModule,
    TranslateModule,
  ],
  template: `
    <div class="create-contract-page">
      <div class="page-header">
        <h1>
          <mat-icon>add_circle</mat-icon>
          {{ 'contracts.create.title' | translate }}
        </h1>
      </div>

      <mat-stepper [linear]="true" #stepper>
        <!-- Step 1: Select Partner -->
        <mat-step [stepControl]="partnerForm" label="{{ 'contracts.create.step1' | translate }}">
          <mat-card class="step-card">
            <mat-card-content>
              <h3>{{ 'contracts.create.selectPartner' | translate }}</h3>
              <form [formGroup]="partnerForm">
                <mat-form-field appearance="outline" class="full-width">
                  <mat-label>{{ 'contracts.create.searchPartner' | translate }}</mat-label>
                  <input matInput
                    formControlName="partnerSearch"
                    [matAutocomplete]="partnerAuto"
                    (input)="onPartnerSearch($event)">
                  <mat-icon matPrefix>search</mat-icon>
                  <mat-autocomplete #partnerAuto="matAutocomplete"
                    (optionSelected)="onPartnerSelected($event.option.value)">
                    @for (partner of partnerResults(); track partner.id) {
                      <mat-option [value]="partner">
                        <div class="partner-option">
                          <mat-icon>{{ partner.role === 'eleveur' ? 'agriculture' : 'store' }}</mat-icon>
                          <div>
                            <span class="partner-name">{{ partner.nom }} {{ partner.prenom || '' }}</span>
                            <span class="partner-info">{{ partner.localisation }} - {{ partner.note | number:'1.1-1' }}/5</span>
                          </div>
                        </div>
                      </mat-option>
                    }
                  </mat-autocomplete>
                </mat-form-field>

                @if (selectedPartner(); as partner) {
                  <mat-card class="selected-partner-card">
                    <mat-card-header>
                      <mat-icon mat-card-avatar class="partner-avatar">
                        {{ partner.role === 'eleveur' ? 'agriculture' : 'store' }}
                      </mat-icon>
                      <mat-card-title>{{ partner.nom }} {{ partner.prenom || '' }}</mat-card-title>
                      <mat-card-subtitle>{{ partner.localisation }} - {{ partner.note | number:'1.1-1' }}/5</mat-card-subtitle>
                    </mat-card-header>
                  </mat-card>
                }

                <input type="hidden" formControlName="partnerId">
              </form>

              <div class="step-actions">
                <button mat-raised-button color="primary" matStepperNext
                  [disabled]="partnerForm.invalid">
                  {{ 'contracts.create.next' | translate }}
                  <mat-icon>arrow_forward</mat-icon>
                </button>
              </div>
            </mat-card-content>
          </mat-card>
        </mat-step>

        <!-- Step 2: Terms -->
        <mat-step [stepControl]="termsForm" label="{{ 'contracts.create.step2' | translate }}">
          <mat-card class="step-card">
            <mat-card-content>
              <h3>{{ 'contracts.create.defineTerms' | translate }}</h3>
              <form [formGroup]="termsForm">
                <div class="form-row">
                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.race' | translate }}</mat-label>
                    <mat-select formControlName="race" required>
                      @for (race of races; track race) {
                        <mat-option [value]="race">{{ race }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>

                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.quantityPerDelivery' | translate }}</mat-label>
                    <input matInput type="number" formControlName="quantityPerDelivery" min="1" required>
                  </mat-form-field>
                </div>

                <div class="form-row">
                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.minimumWeight' | translate }} (kg)</mat-label>
                    <input matInput type="number" formControlName="minimumWeight" min="0" step="0.1" required>
                  </mat-form-field>

                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.pricePerKg' | translate }} (FCFA)</mat-label>
                    <input matInput type="number" formControlName="pricePerKg" min="0" required>
                  </mat-form-field>

                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.priceType' | translate }}</mat-label>
                    <mat-select formControlName="priceType" required>
                      <mat-option value="FIXE">{{ 'contracts.create.priceFixed' | translate }}</mat-option>
                      <mat-option value="INDEXE">{{ 'contracts.create.priceIndexed' | translate }}</mat-option>
                    </mat-select>
                  </mat-form-field>
                </div>
              </form>

              <div class="step-actions">
                <button mat-button matStepperPrevious>
                  <mat-icon>arrow_back</mat-icon>
                  {{ 'contracts.create.previous' | translate }}
                </button>
                <button mat-raised-button color="primary" matStepperNext
                  [disabled]="termsForm.invalid">
                  {{ 'contracts.create.next' | translate }}
                  <mat-icon>arrow_forward</mat-icon>
                </button>
              </div>
            </mat-card-content>
          </mat-card>
        </mat-step>

        <!-- Step 3: Schedule -->
        <mat-step [stepControl]="scheduleForm" label="{{ 'contracts.create.step3' | translate }}">
          <mat-card class="step-card">
            <mat-card-content>
              <h3>{{ 'contracts.create.defineSchedule' | translate }}</h3>
              <form [formGroup]="scheduleForm">
                <div class="form-row">
                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.frequency' | translate }}</mat-label>
                    <mat-select formControlName="frequency" required>
                      @for (freq of frequencies; track freq.value) {
                        <mat-option [value]="freq.value">{{ freq.label }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>

                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.dayPreference' | translate }}</mat-label>
                    <mat-select formControlName="dayPreference">
                      <mat-option [value]="null">{{ 'contracts.create.noPreference' | translate }}</mat-option>
                      @for (day of daysOfWeek; track day.value) {
                        <mat-option [value]="day.value">{{ day.label }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                </div>

                <div class="form-row">
                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.startDate' | translate }}</mat-label>
                    <input matInput [matDatepicker]="startPicker" formControlName="startDate" required>
                    <mat-datepicker-toggle matIconSuffix [for]="startPicker"></mat-datepicker-toggle>
                    <mat-datepicker #startPicker></mat-datepicker>
                  </mat-form-field>

                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.duration' | translate }}</mat-label>
                    <mat-select formControlName="duration" required>
                      @for (dur of durations; track dur.value) {
                        <mat-option [value]="dur.value">{{ dur.label }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                </div>

                @if (scheduleForm.get('startDate')?.value && scheduleForm.get('duration')?.value) {
                  <div class="computed-end">
                    <mat-icon>event</mat-icon>
                    <span>{{ 'contracts.create.endDate' | translate }}: <strong>{{ computedEndDate() | date:'mediumDate' }}</strong></span>
                  </div>
                }
              </form>

              <div class="step-actions">
                <button mat-button matStepperPrevious>
                  <mat-icon>arrow_back</mat-icon>
                  {{ 'contracts.create.previous' | translate }}
                </button>
                <button mat-raised-button color="primary" matStepperNext
                  [disabled]="scheduleForm.invalid">
                  {{ 'contracts.create.next' | translate }}
                  <mat-icon>arrow_forward</mat-icon>
                </button>
              </div>
            </mat-card-content>
          </mat-card>
        </mat-step>

        <!-- Step 4: Conditions -->
        <mat-step [stepControl]="conditionsForm" label="{{ 'contracts.create.step4' | translate }}">
          <mat-card class="step-card">
            <mat-card-content>
              <h3>{{ 'contracts.create.defineConditions' | translate }}</h3>
              <form [formGroup]="conditionsForm">
                <div class="form-row">
                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.advancePayment' | translate }} (%)</mat-label>
                    <input matInput type="number" formControlName="advancePaymentPercent"
                      min="0" max="100" required>
                    <mat-hint>{{ 'contracts.create.advancePaymentHint' | translate }}</mat-hint>
                  </mat-form-field>
                </div>

                <div class="form-row">
                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.penaltyLateDelivery' | translate }} (%)</mat-label>
                    <input matInput type="number" formControlName="penaltyLateDelivery"
                      min="0" max="100" required>
                    <mat-hint>{{ 'contracts.create.penaltyLateHint' | translate }}</mat-hint>
                  </mat-form-field>

                  <mat-form-field appearance="outline" class="form-field">
                    <mat-label>{{ 'contracts.create.penaltyUnderWeight' | translate }} (%)</mat-label>
                    <input matInput type="number" formControlName="penaltyUnderWeight"
                      min="0" max="100" required>
                    <mat-hint>{{ 'contracts.create.penaltyWeightHint' | translate }}</mat-hint>
                  </mat-form-field>
                </div>

                <mat-divider></mat-divider>

                <div class="checkbox-section">
                  <mat-checkbox formControlName="halalRequired">
                    {{ 'contracts.create.halalRequired' | translate }}
                  </mat-checkbox>
                </div>

                <div class="checkbox-section">
                  <mat-checkbox formControlName="veterinaryCertificationRequired">
                    {{ 'contracts.create.vetRequired' | translate }}
                  </mat-checkbox>
                </div>
              </form>

              <div class="step-actions">
                <button mat-button matStepperPrevious>
                  <mat-icon>arrow_back</mat-icon>
                  {{ 'contracts.create.previous' | translate }}
                </button>
                <button mat-raised-button color="primary" matStepperNext
                  [disabled]="conditionsForm.invalid">
                  {{ 'contracts.create.next' | translate }}
                  <mat-icon>arrow_forward</mat-icon>
                </button>
              </div>
            </mat-card-content>
          </mat-card>
        </mat-step>

        <!-- Step 5: Review & Confirm -->
        <mat-step label="{{ 'contracts.create.step5' | translate }}">
          <mat-card class="step-card">
            <mat-card-content>
              <h3>{{ 'contracts.create.reviewConfirm' | translate }}</h3>

              <!-- Partner Summary -->
              <div class="review-section">
                <h4>{{ 'contracts.create.step1' | translate }}</h4>
                @if (selectedPartner(); as p) {
                  <p><strong>{{ p.nom }} {{ p.prenom || '' }}</strong> ({{ p.role }}) - {{ p.localisation }}</p>
                }
              </div>

              <mat-divider></mat-divider>

              <!-- Terms Summary -->
              <div class="review-section">
                <h4>{{ 'contracts.create.step2' | translate }}</h4>
                <div class="review-grid">
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.race' | translate }}:</span>
                    <span>{{ termsForm.get('race')?.value }}</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.quantityPerDelivery' | translate }}:</span>
                    <span>{{ termsForm.get('quantityPerDelivery')?.value }}</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.minimumWeight' | translate }}:</span>
                    <span>{{ termsForm.get('minimumWeight')?.value }} kg</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.pricePerKg' | translate }}:</span>
                    <span class="price">{{ termsForm.get('pricePerKg')?.value | number }} FCFA/kg</span>
                  </div>
                </div>
              </div>

              <mat-divider></mat-divider>

              <!-- Schedule Summary -->
              <div class="review-section">
                <h4>{{ 'contracts.create.step3' | translate }}</h4>
                <div class="review-grid">
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.frequency' | translate }}:</span>
                    <span>{{ scheduleForm.get('frequency')?.value }}</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.startDate' | translate }}:</span>
                    <span>{{ scheduleForm.get('startDate')?.value | date:'mediumDate' }}</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.duration' | translate }}:</span>
                    <span>{{ scheduleForm.get('duration')?.value }}</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.endDate' | translate }}:</span>
                    <span>{{ computedEndDate() | date:'mediumDate' }}</span>
                  </div>
                </div>
              </div>

              <mat-divider></mat-divider>

              <!-- Conditions Summary -->
              <div class="review-section">
                <h4>{{ 'contracts.create.step4' | translate }}</h4>
                <div class="review-grid">
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.advancePayment' | translate }}:</span>
                    <span>{{ conditionsForm.get('advancePaymentPercent')?.value }}%</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.penaltyLateDelivery' | translate }}:</span>
                    <span>{{ conditionsForm.get('penaltyLateDelivery')?.value }}%</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.create.penaltyUnderWeight' | translate }}:</span>
                    <span>{{ conditionsForm.get('penaltyUnderWeight')?.value }}%</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.halal' | translate }}:</span>
                    <span>{{ conditionsForm.get('halalRequired')?.value ? 'Oui' : 'Non' }}</span>
                  </div>
                  <div class="review-item">
                    <span class="review-label">{{ 'contracts.vet' | translate }}:</span>
                    <span>{{ conditionsForm.get('veterinaryCertificationRequired')?.value ? 'Oui' : 'Non' }}</span>
                  </div>
                </div>
              </div>

              <div class="step-actions final">
                <button mat-button matStepperPrevious>
                  <mat-icon>arrow_back</mat-icon>
                  {{ 'contracts.create.previous' | translate }}
                </button>
                <button mat-raised-button color="primary" (click)="onSubmit()"
                  [disabled]="submitting()">
                  @if (submitting()) {
                    <mat-spinner diameter="20"></mat-spinner>
                  } @else {
                    <mat-icon>check</mat-icon>
                    {{ 'contracts.create.confirm' | translate }}
                  }
                </button>
              </div>
            </mat-card-content>
          </mat-card>
        </mat-step>
      </mat-stepper>
    </div>
  `,
  styles: [`
    .create-contract-page {
      padding: 24px;
      max-width: 960px;
      margin: 0 auto;
    }

    .page-header h1 {
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .step-card {
      margin-top: 16px;
    }

    .step-card h3 {
      margin: 0 0 16px;
      color: #333;
    }

    .full-width {
      width: 100%;
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

    .partner-option {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .partner-name {
      display: block;
      font-weight: 500;
    }

    .partner-info {
      display: block;
      font-size: 0.8rem;
      color: #666;
    }

    .selected-partner-card {
      margin-top: 16px;
      border-left: 4px solid #4caf50;
    }

    .partner-avatar {
      color: #2e7d32;
      background: #e8f5e9;
      border-radius: 50%;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .computed-end {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 12px;
      background: #f5f5f5;
      border-radius: 8px;
      margin-top: 12px;
    }

    .computed-end mat-icon {
      color: #1976d2;
    }

    .checkbox-section {
      margin: 12px 0;
    }

    mat-divider {
      margin: 16px 0;
    }

    .step-actions {
      display: flex;
      justify-content: flex-end;
      gap: 12px;
      margin-top: 24px;
      padding-top: 16px;
      border-top: 1px solid rgba(0, 0, 0, 0.12);
    }

    .step-actions.final {
      justify-content: space-between;
    }

    /* Review */
    .review-section {
      margin: 16px 0;
    }

    .review-section h4 {
      margin: 0 0 8px;
      color: #1976d2;
    }

    .review-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
      gap: 8px;
    }

    .review-item {
      display: flex;
      gap: 8px;
    }

    .review-label {
      color: #666;
      font-size: 0.9rem;
    }

    .price {
      color: #2e7d32;
      font-weight: 600;
    }
  `],
})
export class CreateContractComponent {
  private readonly contractsService = inject(ContractsService);
  private readonly fb = inject(FormBuilder);
  private readonly router = inject(Router);
  private readonly snackBar = inject(MatSnackBar);

  readonly races = CHICKEN_RACES;
  readonly daysOfWeek = DAYS_OF_WEEK;
  readonly submitting = signal(false);
  readonly partnerResults = signal<PartnerSearchResult[]>([]);
  readonly selectedPartner = signal<PartnerSearchResult | null>(null);

  readonly frequencies: { value: ContractFrequency; label: string }[] = [
    { value: 'HEBDOMADAIRE', label: 'Hebdomadaire' },
    { value: 'BI_MENSUEL', label: 'Bi-mensuel' },
    { value: 'MENSUEL', label: 'Mensuel' },
  ];

  readonly durations: { value: ContractDuration; label: string }[] = [
    { value: '3_MOIS', label: '3 mois' },
    { value: '6_MOIS', label: '6 mois' },
    { value: '12_MOIS', label: '12 mois' },
  ];

  // Step 1: Partner
  readonly partnerForm: FormGroup = this.fb.group({
    partnerSearch: [''],
    partnerId: ['', Validators.required],
  });

  // Step 2: Terms
  readonly termsForm: FormGroup = this.fb.group({
    race: ['', Validators.required],
    quantityPerDelivery: [null, [Validators.required, Validators.min(1)]],
    minimumWeight: [null, [Validators.required, Validators.min(0)]],
    pricePerKg: [null, [Validators.required, Validators.min(0)]],
    priceType: ['FIXE', Validators.required],
  });

  // Step 3: Schedule
  readonly scheduleForm: FormGroup = this.fb.group({
    frequency: ['HEBDOMADAIRE', Validators.required],
    dayPreference: [null],
    startDate: [null, Validators.required],
    duration: ['3_MOIS', Validators.required],
  });

  // Step 4: Conditions
  readonly conditionsForm: FormGroup = this.fb.group({
    advancePaymentPercent: [0, [Validators.required, Validators.min(0), Validators.max(100)]],
    penaltyLateDelivery: [5, [Validators.required, Validators.min(0), Validators.max(100)]],
    penaltyUnderWeight: [3, [Validators.required, Validators.min(0), Validators.max(100)]],
    halalRequired: [false],
    veterinaryCertificationRequired: [false],
  });

  computedEndDate(): Date | null {
    const start = this.scheduleForm.get('startDate')?.value;
    const duration = this.scheduleForm.get('duration')?.value;
    if (!start || !duration) return null;
    return computeEndDate(new Date(start).toISOString(), duration);
  }

  onPartnerSearch(event: Event): void {
    const query = (event.target as HTMLInputElement).value;
    if (query && query.length >= 2) {
      this.contractsService.searchPartners(query).subscribe({
        next: (results) => this.partnerResults.set(results),
      });
    }
  }

  onPartnerSelected(partner: PartnerSearchResult): void {
    this.selectedPartner.set(partner);
    this.partnerForm.patchValue({
      partnerSearch: `${partner.nom} ${partner.prenom || ''}`,
      partnerId: partner.id,
    });
  }

  onSubmit(): void {
    this.submitting.set(true);
    const terms = this.termsForm.value;
    const schedule = this.scheduleForm.value;
    const conditions = this.conditionsForm.value;

    const input: CreateContractInput = {
      partnerId: this.partnerForm.get('partnerId')!.value,
      race: terms.race,
      quantityPerDelivery: terms.quantityPerDelivery,
      minimumWeight: terms.minimumWeight,
      pricePerKg: terms.pricePerKg,
      priceType: terms.priceType,
      frequency: schedule.frequency,
      dayPreference: schedule.dayPreference,
      startDate: new Date(schedule.startDate).toISOString(),
      duration: schedule.duration,
      advancePaymentPercent: conditions.advancePaymentPercent,
      penaltyLateDelivery: conditions.penaltyLateDelivery,
      penaltyUnderWeight: conditions.penaltyUnderWeight,
      halalRequired: conditions.halalRequired,
      veterinaryCertificationRequired: conditions.veterinaryCertificationRequired,
    };

    this.contractsService.createContract(input).subscribe({
      next: (contract) => {
        this.submitting.set(false);
        this.snackBar.open('Contrat cree avec succes', 'OK', { duration: 3000 });
        this.router.navigate(['/contracts', contract.id]);
      },
      error: () => {
        this.submitting.set(false);
        this.snackBar.open('Erreur lors de la creation du contrat', 'OK', { duration: 3000 });
      },
    });
  }
}
