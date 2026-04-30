import { Component, OnInit, signal, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { Router, RouterLink } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { TranslateModule } from '@ngx-translate/core';
import { AuthService } from '@services/auth.service';

@Component({
  selector: 'app-profile-edit',
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
    TranslateModule,
  ],
  template: `
    <div class="edit-container" data-testid="profile-edit-page">
      <div class="page-header">
        <button mat-icon-button routerLink=".." data-testid="profile-edit-action-back">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'profile.edit.title' | translate }}</h1>
      </div>

      <mat-card>
        <mat-card-content>
          <form [formGroup]="form" (ngSubmit)="onSubmit()" class="edit-form" data-testid="profile-edit-form">
            <!-- Avatar Upload -->
            <div class="avatar-section">
              <div class="avatar-preview">
                @if (avatarPreview()) {
                  <img [src]="avatarPreview()" alt="Avatar">
                } @else {
                  <mat-icon>person</mat-icon>
                }
              </div>
              <button mat-stroked-button type="button" (click)="fileInput.click()"
                      data-testid="profile-edit-action-change-avatar">
                <mat-icon>photo_camera</mat-icon>
                {{ 'profile.edit.change_avatar' | translate }}
              </button>
              <input #fileInput type="file" accept="image/*" hidden
                     (change)="onAvatarChange($event)"
                     data-testid="profile-edit-form-photo">
            </div>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'profile.edit.name' | translate }}</mat-label>
              <input matInput formControlName="nom" data-testid="profile-edit-form-name">
              @if (form.get('nom')?.hasError('required')) {
                <mat-error data-testid="profile-edit-form-error-name">{{ 'profile.edit.name_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'profile.edit.phone' | translate }}</mat-label>
              <input matInput formControlName="phone" placeholder="+226 XX XX XX XX"
                     data-testid="profile-edit-form-phone">
              @if (form.get('phone')?.hasError('required')) {
                <mat-error data-testid="profile-edit-form-error-phone">{{ 'profile.edit.phone_required' | translate }}</mat-error>
              }
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'profile.edit.location' | translate }}</mat-label>
              <input matInput formControlName="localisation" data-testid="profile-edit-form-address">
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'profile.edit.description' | translate }}</mat-label>
              <textarea matInput formControlName="description" rows="4"
                        data-testid="profile-edit-form-description"></textarea>
            </mat-form-field>

            <div class="form-actions">
              <button mat-button type="button" routerLink=".."
                      data-testid="profile-edit-action-cancel">
                {{ 'common.cancel' | translate }}
              </button>
              <button mat-raised-button color="primary" type="submit"
                      [disabled]="form.invalid || submitting()"
                      data-testid="profile-edit-form-submit">
                <mat-icon>save</mat-icon>
                {{ 'profile.edit.save' | translate }}
              </button>
            </div>
          </form>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .edit-container {
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

    .edit-form {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
    }

    .avatar-section {
      grid-column: 1 / -1;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 12px;
      padding: 16px 0;
    }

    .avatar-preview {
      width: 80px;
      height: 80px;
      border-radius: 50%;
      background: #e0e0e0;
      display: flex;
      align-items: center;
      justify-content: center;
      overflow: hidden;

      mat-icon { font-size: 40px; width: 40px; height: 40px; color: #999; }
      img { width: 100%; height: 100%; object-fit: cover; }
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
export class ProfileEditComponent implements OnInit {
  private readonly fb = new FormBuilder();
  private readonly auth = inject(AuthService);
  readonly submitting = signal(false);
  readonly avatarPreview = signal<string | null>(null);

  readonly form = this.fb.nonNullable.group({
    nom: ['', Validators.required],
    phone: ['', Validators.required],
    localisation: [''],
    description: [''],
  });

  constructor(private readonly router: Router) {}

  ngOnInit(): void {
    const user = this.auth.currentUser();
    this.form.patchValue({
      nom: user?.name || 'Ouedraogo Moussa',
      phone: '+226 70 12 34 56',
      localisation: 'Koudougou, Burkina Faso',
      description: 'Eleveur professionnel specialise dans le poulet bicyclette et la pintade.',
    });
  }

  onAvatarChange(event: Event): void {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onload = () => this.avatarPreview.set(reader.result as string);
      reader.readAsDataURL(file);
    }
  }

  onSubmit(): void {
    if (this.form.invalid) return;
    this.submitting.set(true);
    console.log('Profile updated:', this.form.value);
    // TODO: API call
    this.router.navigate(['/profile']);
  }
}
