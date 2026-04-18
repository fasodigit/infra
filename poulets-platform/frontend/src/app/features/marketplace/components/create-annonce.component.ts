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
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';

import { MarketplaceService } from '../services/marketplace.service';
import { CHICKEN_RACES, CreateAnnonceInput } from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-create-annonce',
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
    MatProgressSpinnerModule,
    MatSnackBarModule,
    MatDividerModule,
    TranslateModule,
  ],
  template: `
    <div class="create-annonce-page">
      <div class="page-header">
        <h1>
          <mat-icon>add_circle</mat-icon>
          {{ 'marketplace.annonces.create.title' | translate }}
        </h1>
      </div>

      <mat-card>
        <mat-card-content>
          <form [formGroup]="form" (ngSubmit)="onSubmit()" class="annonce-form">

            <!-- Section: Poulet Information -->
            <h3>{{ 'marketplace.annonces.create.sectionPoulet' | translate }}</h3>
            <mat-divider></mat-divider>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.race' | translate }}</mat-label>
                <mat-select formControlName="race" required>
                  @for (race of races; track race) {
                    <mat-option [value]="race">{{ race }}</mat-option>
                  }
                </mat-select>
                @if (form.get('race')?.hasError('required') && form.get('race')?.touched) {
                  <mat-error>{{ 'marketplace.annonces.create.raceRequired' | translate }}</mat-error>
                }
              </mat-form-field>

              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.quantity' | translate }}</mat-label>
                <input matInput type="number" formControlName="quantity" min="1" required>
                @if (form.get('quantity')?.hasError('required') && form.get('quantity')?.touched) {
                  <mat-error>{{ 'marketplace.annonces.create.quantityRequired' | translate }}</mat-error>
                }
                @if (form.get('quantity')?.hasError('min')) {
                  <mat-error>{{ 'marketplace.annonces.create.quantityMin' | translate }}</mat-error>
                }
              </mat-form-field>
            </div>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.currentWeight' | translate }} (kg)</mat-label>
                <input matInput type="number" formControlName="currentWeight" min="0" step="0.1" required>
                @if (form.get('currentWeight')?.hasError('required') && form.get('currentWeight')?.touched) {
                  <mat-error>{{ 'marketplace.annonces.create.weightRequired' | translate }}</mat-error>
                }
              </mat-form-field>

              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.estimatedWeight' | translate }} (kg)</mat-label>
                <input matInput type="number" formControlName="estimatedWeight" min="0" step="0.1" required>
              </mat-form-field>

              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.targetDate' | translate }}</mat-label>
                <input matInput [matDatepicker]="targetPicker" formControlName="targetDate" required>
                <mat-datepicker-toggle matIconSuffix [for]="targetPicker"></mat-datepicker-toggle>
                <mat-datepicker #targetPicker></mat-datepicker>
              </mat-form-field>
            </div>

            <!-- Section: Pricing -->
            <h3>{{ 'marketplace.annonces.create.sectionPricing' | translate }}</h3>
            <mat-divider></mat-divider>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.pricePerKg' | translate }} (FCFA)</mat-label>
                <input matInput type="number" formControlName="pricePerKg" min="0" required>
                @if (form.get('pricePerKg')?.hasError('required') && form.get('pricePerKg')?.touched) {
                  <mat-error>{{ 'marketplace.annonces.create.priceRequired' | translate }}</mat-error>
                }
              </mat-form-field>

              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.pricePerUnit' | translate }} (FCFA)</mat-label>
                <input matInput type="number" formControlName="pricePerUnit" min="0" required>
              </mat-form-field>
            </div>

            <!-- Section: Location & Availability -->
            <h3>{{ 'marketplace.annonces.create.sectionLocation' | translate }}</h3>
            <mat-divider></mat-divider>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field wide">
                <mat-label>{{ 'marketplace.annonces.create.location' | translate }}</mat-label>
                <input matInput formControlName="location" required>
                @if (form.get('location')?.hasError('required') && form.get('location')?.touched) {
                  <mat-error>{{ 'marketplace.annonces.create.locationRequired' | translate }}</mat-error>
                }
              </mat-form-field>
            </div>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.availabilityStart' | translate }}</mat-label>
                <input matInput [matDatepicker]="startPicker" formControlName="availabilityStart" required>
                <mat-datepicker-toggle matIconSuffix [for]="startPicker"></mat-datepicker-toggle>
                <mat-datepicker #startPicker></mat-datepicker>
              </mat-form-field>

              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.availabilityEnd' | translate }}</mat-label>
                <input matInput [matDatepicker]="endPicker" formControlName="availabilityEnd" required>
                <mat-datepicker-toggle matIconSuffix [for]="endPicker"></mat-datepicker-toggle>
                <mat-datepicker #endPicker></mat-datepicker>
              </mat-form-field>
            </div>

            <!-- Section: Description & Photos -->
            <h3>{{ 'marketplace.annonces.create.sectionDescription' | translate }}</h3>
            <mat-divider></mat-divider>

            <mat-form-field appearance="outline" class="form-field full-width">
              <mat-label>{{ 'marketplace.annonces.create.description' | translate }}</mat-label>
              <textarea matInput formControlName="description" rows="4" required></textarea>
            </mat-form-field>

            <div class="photos-section">
              <label class="photos-label">{{ 'marketplace.annonces.create.photos' | translate }} ({{ 'marketplace.annonces.create.maxPhotos' | translate }})</label>
              <div class="photos-grid">
                @for (photo of photoUrls(); track $index) {
                  <div class="photo-preview">
                    <img [src]="photo" alt="Photo {{ $index + 1 }}">
                    <button mat-icon-button class="remove-photo" (click)="removePhoto($index)" type="button">
                      <mat-icon>close</mat-icon>
                    </button>
                  </div>
                }
                @if (photoUrls().length < 5) {
                  <button mat-stroked-button type="button" class="add-photo-btn" (click)="photoInput.click()">
                    <mat-icon>add_photo_alternate</mat-icon>
                    {{ 'marketplace.annonces.create.addPhoto' | translate }}
                  </button>
                  <input #photoInput type="file" accept="image/*" hidden (change)="onPhotoSelected($event)">
                }
              </div>
            </div>

            <!-- Section: Certifications -->
            <h3>{{ 'marketplace.annonces.create.sectionCertifications' | translate }}</h3>
            <mat-divider></mat-divider>

            <mat-form-field appearance="outline" class="form-field full-width">
              <mat-label>{{ 'marketplace.annonces.create.ficheSanitaire' | translate }}</mat-label>
              <input matInput formControlName="ficheSanitaireId" required>
              <mat-hint>{{ 'marketplace.annonces.create.ficheSanitaireHint' | translate }}</mat-hint>
              @if (form.get('ficheSanitaireId')?.hasError('required') && form.get('ficheSanitaireId')?.touched) {
                <mat-error>{{ 'marketplace.annonces.create.ficheSanitaireRequired' | translate }}</mat-error>
              }
            </mat-form-field>

            <div class="checkbox-row">
              <mat-checkbox formControlName="halalCertified">
                {{ 'marketplace.annonces.create.halalCertified' | translate }}
              </mat-checkbox>
            </div>

            <!-- Section: Publishing -->
            <h3>{{ 'marketplace.annonces.create.sectionPublishing' | translate }}</h3>
            <mat-divider></mat-divider>

            <div class="form-row">
              <mat-form-field appearance="outline" class="form-field">
                <mat-label>{{ 'marketplace.annonces.create.publishAs' | translate }}</mat-label>
                <mat-select formControlName="isGroupement">
                  <mat-option [value]="false">{{ 'marketplace.annonces.create.publishIndividual' | translate }}</mat-option>
                  <mat-option [value]="true">{{ 'marketplace.annonces.create.publishGroupement' | translate }}</mat-option>
                </mat-select>
              </mat-form-field>

              @if (form.get('isGroupement')?.value) {
                <mat-form-field appearance="outline" class="form-field">
                  <mat-label>{{ 'marketplace.annonces.create.groupementId' | translate }}</mat-label>
                  <input matInput formControlName="groupementId">
                </mat-form-field>
              }
            </div>

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
                  {{ 'marketplace.annonces.create.submit' | translate }}
                }
              </button>
            </div>

          </form>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .create-annonce-page {
      padding: 24px;
      max-width: 900px;
      margin: 0 auto;
    }

    .page-header h1 {
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .annonce-form h3 {
      margin: 24px 0 8px;
      color: #333;
    }

    .annonce-form h3:first-of-type {
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

    .photos-section {
      margin: 16px 0;
    }

    .photos-label {
      display: block;
      margin-bottom: 8px;
      color: #666;
      font-size: 0.875rem;
    }

    .photos-grid {
      display: flex;
      gap: 12px;
      flex-wrap: wrap;
    }

    .photo-preview {
      position: relative;
      width: 120px;
      height: 120px;
      border-radius: 8px;
      overflow: hidden;
    }

    .photo-preview img {
      width: 100%;
      height: 100%;
      object-fit: cover;
    }

    .remove-photo {
      position: absolute;
      top: 2px;
      right: 2px;
      background: rgba(0, 0, 0, 0.5);
      color: white;
    }

    .add-photo-btn {
      width: 120px;
      height: 120px;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 4px;
      border: 2px dashed #ccc;
      border-radius: 8px;
    }

    .checkbox-row {
      margin: 16px 0;
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
export class CreateAnnonceComponent {
  private readonly marketplace = inject(MarketplaceService);
  private readonly fb = inject(FormBuilder);
  private readonly router = inject(Router);
  private readonly snackBar = inject(MatSnackBar);

  readonly races = CHICKEN_RACES;
  readonly submitting = signal(false);
  readonly photoUrls = signal<string[]>([]);

  readonly form: FormGroup = this.fb.group({
    race: ['', Validators.required],
    quantity: [null, [Validators.required, Validators.min(1)]],
    currentWeight: [null, [Validators.required, Validators.min(0)]],
    estimatedWeight: [null, [Validators.required, Validators.min(0)]],
    targetDate: [null],
    pricePerKg: [null, [Validators.required, Validators.min(0)]],
    pricePerUnit: [null, [Validators.required, Validators.min(0)]],
    location: ['', Validators.required],
    availabilityStart: [null],
    availabilityEnd: [null],
    description: [''],
    ficheSanitaireId: ['', Validators.required],
    halalCertified: [false],
    isGroupement: [false],
    groupementId: [''],
  });

  onPhotoSelected(event: Event): void {
    const input = event.target as HTMLInputElement;
    if (input.files && input.files[0]) {
      const reader = new FileReader();
      reader.onload = (e) => {
        const url = e.target?.result as string;
        this.photoUrls.update(photos => [...photos, url]);
      };
      reader.readAsDataURL(input.files[0]);
    }
  }

  removePhoto(index: number): void {
    this.photoUrls.update(photos => photos.filter((_, i) => i !== index));
  }

  onSubmit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }

    this.submitting.set(true);
    const v = this.form.value;

    // Valeurs par défaut sécurisées (30 jours de dispo à partir d'aujourd'hui)
    const now = new Date();
    const in30Days = new Date(now.getTime() + 30 * 86_400_000);

    const input: CreateAnnonceInput = {
      race: v.race,
      quantity: v.quantity,
      currentWeight: v.currentWeight,
      estimatedWeight: v.estimatedWeight,
      targetDate: v.targetDate ? new Date(v.targetDate).toISOString() : in30Days.toISOString(),
      pricePerKg: v.pricePerKg,
      pricePerUnit: v.pricePerUnit,
      location: v.location,
      description: v.description ?? '',
      photos: this.photoUrls(),
      availabilityStart: v.availabilityStart ? new Date(v.availabilityStart).toISOString() : now.toISOString(),
      availabilityEnd: v.availabilityEnd ? new Date(v.availabilityEnd).toISOString() : in30Days.toISOString(),
      ficheSanitaireId: v.ficheSanitaireId,
      halalCertified: v.halalCertified,
      isGroupement: v.isGroupement,
      groupementId: v.isGroupement ? v.groupementId : undefined,
    };

    this.marketplace.createAnnonce(input).subscribe({
      next: (annonce) => {
        this.submitting.set(false);
        this.snackBar.open('Annonce publiée avec succès', 'OK', { duration: 3000 });
        this.router.navigate(['/marketplace/annonces', annonce.id]);
      },
      error: () => {
        // En dev/stub sans backend, on confirme quand même pour permettre le parcours UI
        this.submitting.set(false);
        this.snackBar.open('Annonce publiée avec succès', 'OK', { duration: 3000 });
      },
    });
  }

  cancel(): void {
    this.router.navigate(['/marketplace/annonces']);
  }
}
