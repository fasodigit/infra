import { Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { TranslateModule } from '@ngx-translate/core';

@Component({
  selector: 'app-add-weight',
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
    TranslateModule,
  ],
  template: `
    <div class="add-weight-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'growth.add_weight.title' | translate }}</h1>
      </div>

      <mat-card>
        <mat-card-content>
          <form [formGroup]="form" (ngSubmit)="onSubmit()" class="weight-form">
            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'growth.add_weight.date' | translate }}</mat-label>
              <input matInput [matDatepicker]="picker" formControlName="date">
              <mat-datepicker-toggle matIconSuffix [for]="picker"></mat-datepicker-toggle>
              <mat-datepicker #picker></mat-datepicker>
              @if (form.get('date')?.hasError('required')) {
                <mat-error>{{ 'growth.add_weight.date_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'growth.add_weight.avg_weight' | translate }} (kg)</mat-label>
              <input matInput type="number" formControlName="poidsMoyen" step="0.01" min="0">
              @if (form.get('poidsMoyen')?.hasError('required')) {
                <mat-error>{{ 'growth.add_weight.weight_required' | translate }}</mat-error>
              }
              @if (form.get('poidsMoyen')?.hasError('min')) {
                <mat-error>{{ 'growth.add_weight.weight_positive' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'growth.add_weight.count' | translate }}</mat-label>
              <input matInput type="number" formControlName="effectif" min="1">
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'growth.add_weight.feed_consumed' | translate }} (kg)</mat-label>
              <input matInput type="number" formControlName="alimentConsomme" min="0">
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'growth.add_weight.notes' | translate }}</mat-label>
              <textarea matInput formControlName="observations" rows="3"></textarea>
            </mat-form-field>

            <div class="form-actions">
              <button mat-button type="button" routerLink="..">
                {{ 'common.cancel' | translate }}
              </button>
              <button mat-raised-button color="primary" type="submit"
                      [disabled]="form.invalid || submitting()">
                <mat-icon>save</mat-icon>
                {{ 'growth.add_weight.save' | translate }}
              </button>
            </div>
          </form>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .add-weight-container {
      padding: 24px;
      max-width: 600px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .weight-form {
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
export class AddWeightComponent {
  private readonly fb = new FormBuilder();
  readonly submitting = signal(false);

  readonly form = this.fb.nonNullable.group({
    date: ['', Validators.required],
    poidsMoyen: [0, [Validators.required, Validators.min(0.01)]],
    effectif: [0],
    alimentConsomme: [0],
    observations: [''],
  });

  constructor(
    private readonly route: ActivatedRoute,
    private readonly router: Router,
  ) {}

  onSubmit(): void {
    if (this.form.invalid) return;
    this.submitting.set(true);
    const lotId = this.route.snapshot.paramMap.get('lotId');
    console.log('Adding weight for lot:', lotId, this.form.value);
    // TODO: API call
    this.router.navigate(['..'], { relativeTo: this.route });
  }
}
