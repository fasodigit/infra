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
  selector: 'app-request-certification',
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
        <h1>{{ 'halal.request.title' | translate }}</h1>
      </div>

      <mat-card>
        <mat-card-content>
          <form [formGroup]="form" (ngSubmit)="onSubmit()" class="request-form">
            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'halal.request.abattoir' | translate }}</mat-label>
              <mat-select formControlName="abattoirId">
                @for (ab of abattoirs; track ab.id) {
                  <mat-option [value]="ab.id">{{ ab.nom }} - {{ ab.adresse }}</mat-option>
                }
              </mat-select>
              @if (form.get('abattoirId')?.hasError('required')) {
                <mat-error>{{ 'halal.request.abattoir_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'halal.request.planned_date' | translate }}</mat-label>
              <input matInput [matDatepicker]="picker" formControlName="datePrevue">
              <mat-datepicker-toggle matIconSuffix [for]="picker"></mat-datepicker-toggle>
              <mat-datepicker #picker></mat-datepicker>
              @if (form.get('datePrevue')?.hasError('required')) {
                <mat-error>{{ 'halal.request.date_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'halal.request.lot' | translate }}</mat-label>
              <mat-select formControlName="lotId">
                @for (lot of availableLots; track lot.id) {
                  <mat-option [value]="lot.id">{{ lot.nom }}</mat-option>
                }
              </mat-select>
              @if (form.get('lotId')?.hasError('required')) {
                <mat-error>{{ 'halal.request.lot_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'halal.request.notes' | translate }}</mat-label>
              <textarea matInput formControlName="observations" rows="3"></textarea>
            </mat-form-field>

            <div class="info-box">
              <mat-icon>info</mat-icon>
              <span>{{ 'halal.request.info_text' | translate }}</span>
            </div>

            <div class="form-actions">
              <button mat-button type="button" routerLink="..">
                {{ 'common.cancel' | translate }}
              </button>
              <button mat-raised-button color="primary" type="submit"
                      [disabled]="form.invalid || submitting()">
                <mat-icon>send</mat-icon>
                {{ 'halal.request.submit' | translate }}
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

    .request-form {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
    }

    .full-width { grid-column: 1 / -1; }

    .info-box {
      grid-column: 1 / -1;
      display: flex;
      align-items: flex-start;
      gap: 12px;
      padding: 12px 16px;
      background: #e3f2fd;
      border-radius: 8px;
      font-size: 0.875rem;
      color: #1565c0;

      mat-icon { color: #1565c0; min-width: 24px; }
    }

    .form-actions {
      grid-column: 1 / -1;
      display: flex;
      justify-content: flex-end;
      gap: 12px;
      padding-top: 8px;
    }
  `],
})
export class RequestCertificationComponent {
  private readonly fb = new FormBuilder();
  readonly submitting = signal(false);

  readonly abattoirs = [
    { id: 'ab1', nom: 'Abattoir Moderne de Ouagadougou', adresse: 'Zone Industrielle' },
    { id: 'ab2', nom: 'Abattoir de Bobo-Dioulasso', adresse: 'Route de Sikasso' },
    { id: 'ab3', nom: 'Abattoir de Koudougou', adresse: 'Secteur 5' },
  ];

  readonly availableLots = [
    { id: 'lot-1', nom: 'Lot A - Brahma (195 tetes)' },
    { id: 'lot-2', nom: 'Lot B - Bicyclette (148 tetes)' },
  ];

  readonly form = this.fb.nonNullable.group({
    abattoirId: ['', Validators.required],
    datePrevue: ['', Validators.required],
    lotId: ['', Validators.required],
    observations: [''],
  });

  constructor(private readonly router: Router) {}

  onSubmit(): void {
    if (this.form.invalid) return;
    this.submitting.set(true);
    console.log('Certification requested:', this.form.value);
    // TODO: API call
    this.router.navigate(['/halal']);
  }
}
