import { Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { Router, RouterLink } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatSelectModule } from '@angular/material/select';
import { TranslateModule } from '@ngx-translate/core';

@Component({
  selector: 'app-add-vaccination',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    ReactiveFormsModule,
    MatCardModule,
    MatFormFieldModule,
    MatInputModule,
    MatButtonModule,
    MatIconModule,
    MatDatepickerModule,
    MatSelectModule,
    TranslateModule,
  ],
  template: `
    <div class="form-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'veterinary.vaccination.title' | translate }}</h1>
      </div>

      <mat-card>
        <mat-card-content>
          <form [formGroup]="form" (ngSubmit)="onSubmit()" class="vaccination-form">
            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'veterinary.vaccination.lot' | translate }}</mat-label>
              <mat-select formControlName="lotId">
                @for (lot of availableLots; track lot.id) {
                  <mat-option [value]="lot.id">{{ lot.nom }}</mat-option>
                }
              </mat-select>
              @if (form.get('lotId')?.hasError('required')) {
                <mat-error>{{ 'veterinary.vaccination.lot_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'veterinary.vaccination.vaccine_name' | translate }}</mat-label>
              <mat-select formControlName="nomVaccin">
                @for (vax of commonVaccines; track vax) {
                  <mat-option [value]="vax">{{ vax }}</mat-option>
                }
              </mat-select>
              @if (form.get('nomVaccin')?.hasError('required')) {
                <mat-error>{{ 'veterinary.vaccination.vaccine_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'veterinary.vaccination.date' | translate }}</mat-label>
              <input matInput [matDatepicker]="picker" formControlName="dateAdministration">
              <mat-datepicker-toggle matIconSuffix [for]="picker"></mat-datepicker-toggle>
              <mat-datepicker #picker></mat-datepicker>
              @if (form.get('dateAdministration')?.hasError('required')) {
                <mat-error>{{ 'veterinary.vaccination.date_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'veterinary.vaccination.vet_name' | translate }}</mat-label>
              <input matInput formControlName="administrePar">
              @if (form.get('administrePar')?.hasError('required')) {
                <mat-error>{{ 'veterinary.vaccination.vet_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'veterinary.vaccination.batch_number' | translate }}</mat-label>
              <input matInput formControlName="lotNumero">
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'veterinary.vaccination.next_dose' | translate }}</mat-label>
              <input matInput [matDatepicker]="nextPicker" formControlName="prochaineDose">
              <mat-datepicker-toggle matIconSuffix [for]="nextPicker"></mat-datepicker-toggle>
              <mat-datepicker #nextPicker></mat-datepicker>
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'veterinary.vaccination.observations' | translate }}</mat-label>
              <textarea matInput formControlName="observations" rows="3"></textarea>
            </mat-form-field>

            <div class="form-actions">
              <button mat-button type="button" routerLink="..">
                {{ 'common.cancel' | translate }}
              </button>
              <button mat-raised-button color="primary" type="submit"
                      [disabled]="form.invalid || submitting()">
                <mat-icon>save</mat-icon>
                {{ 'veterinary.vaccination.save' | translate }}
              </button>
            </div>
          </form>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .form-container {
      padding: 24px;
      max-width: 700px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .vaccination-form {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
    }

    .full-width { grid-column: 1 / -1; }

    .form-actions {
      grid-column: 1 / -1;
      display: flex;
      justify-content: flex-end;
      gap: 12px;
      padding-top: 8px;
    }
  `],
})
export class AddVaccinationComponent {
  private readonly fb = new FormBuilder();
  readonly submitting = signal(false);

  readonly commonVaccines = [
    'Newcastle (La Sota)',
    'Newcastle (HB1)',
    'Gumboro (IBD)',
    'Bronchite infectieuse',
    'Variole aviaire',
    'Marek',
    'Encephalomyelite aviaire',
    'Salmonellose',
  ];

  readonly availableLots = [
    { id: 'lot-1', nom: 'Lot A - Brahma' },
    { id: 'lot-2', nom: 'Lot B - Bicyclette' },
    { id: 'lot-3', nom: 'Lot C - Pintade' },
  ];

  readonly form = this.fb.nonNullable.group({
    lotId: ['', Validators.required],
    nomVaccin: ['', Validators.required],
    dateAdministration: ['', Validators.required],
    administrePar: ['', Validators.required],
    lotNumero: [''],
    prochaineDose: [''],
    observations: [''],
  });

  constructor(private readonly router: Router) {}

  onSubmit(): void {
    if (this.form.invalid) return;
    this.submitting.set(true);
    console.log('Vaccination submitted:', this.form.value);
    // TODO: API call
    this.router.navigate(['/veterinary']);
  }
}
