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
  selector: 'app-add-treatment',
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
        <h1>{{ 'veterinary.treatment.title' | translate }}</h1>
      </div>

      <mat-card>
        <mat-card-content>
          <form [formGroup]="form" (ngSubmit)="onSubmit()" class="treatment-form">
            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'veterinary.treatment.lot' | translate }}</mat-label>
              <mat-select formControlName="lotId">
                @for (lot of availableLots; track lot.id) {
                  <mat-option [value]="lot.id">{{ lot.nom }}</mat-option>
                }
              </mat-select>
              @if (form.get('lotId')?.hasError('required')) {
                <mat-error>{{ 'veterinary.treatment.lot_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'veterinary.treatment.disease' | translate }}</mat-label>
              <input matInput formControlName="diagnostic">
              @if (form.get('diagnostic')?.hasError('required')) {
                <mat-error>{{ 'veterinary.treatment.disease_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'veterinary.treatment.treatment_name' | translate }}</mat-label>
              <input matInput formControlName="nomTraitement">
              @if (form.get('nomTraitement')?.hasError('required')) {
                <mat-error>{{ 'veterinary.treatment.treatment_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'veterinary.treatment.start_date' | translate }}</mat-label>
              <input matInput [matDatepicker]="startPicker" formControlName="dateDebut">
              <mat-datepicker-toggle matIconSuffix [for]="startPicker"></mat-datepicker-toggle>
              <mat-datepicker #startPicker></mat-datepicker>
              @if (form.get('dateDebut')?.hasError('required')) {
                <mat-error>{{ 'veterinary.treatment.start_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'veterinary.treatment.end_date' | translate }}</mat-label>
              <input matInput [matDatepicker]="endPicker" formControlName="dateFin">
              <mat-datepicker-toggle matIconSuffix [for]="endPicker"></mat-datepicker-toggle>
              <mat-datepicker #endPicker></mat-datepicker>
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'veterinary.treatment.prescribed_by' | translate }}</mat-label>
              <input matInput formControlName="prescritPar">
              @if (form.get('prescritPar')?.hasError('required')) {
                <mat-error>{{ 'veterinary.treatment.vet_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'veterinary.treatment.notes' | translate }}</mat-label>
              <textarea matInput formControlName="observations" rows="3"></textarea>
            </mat-form-field>

            <div class="form-actions">
              <button mat-button type="button" routerLink="..">
                {{ 'common.cancel' | translate }}
              </button>
              <button mat-raised-button color="primary" type="submit"
                      [disabled]="form.invalid || submitting()">
                <mat-icon>save</mat-icon>
                {{ 'veterinary.treatment.save' | translate }}
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

    .treatment-form {
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
export class AddTreatmentComponent {
  private readonly fb = new FormBuilder();
  readonly submitting = signal(false);

  readonly availableLots = [
    { id: 'lot-1', nom: 'Lot A - Brahma' },
    { id: 'lot-2', nom: 'Lot B - Bicyclette' },
    { id: 'lot-3', nom: 'Lot C - Pintade' },
  ];

  readonly form = this.fb.nonNullable.group({
    lotId: ['', Validators.required],
    diagnostic: ['', Validators.required],
    nomTraitement: ['', Validators.required],
    dateDebut: ['', Validators.required],
    dateFin: [''],
    prescritPar: ['', Validators.required],
    observations: [''],
  });

  constructor(private readonly router: Router) {}

  onSubmit(): void {
    if (this.form.invalid) return;
    this.submitting.set(true);
    console.log('Treatment submitted:', this.form.value);
    // TODO: API call
    this.router.navigate(['/veterinary']);
  }
}
