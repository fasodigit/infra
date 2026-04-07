import { Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, Validators } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';
import { RatingStarsComponent } from '@shared/components/rating-stars/rating-stars.component';

@Component({
  selector: 'app-leave-review',
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
    MatDividerModule,
    TranslateModule,
    RatingStarsComponent,
  ],
  template: `
    <div class="review-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'reputation.review.title' | translate }}</h1>
      </div>

      <mat-card>
        <mat-card-content>
          <form [formGroup]="form" (ngSubmit)="onSubmit()" class="review-form">
            <!-- Overall Rating -->
            <div class="rating-section">
              <label>{{ 'reputation.review.overall_rating' | translate }}</label>
              <app-rating-stars
                [value]="overallRating"
                [interactive]="true"
                (ratingChange)="onRatingChange($event)">
              </app-rating-stars>
              @if (submitted && overallRating === 0) {
                <span class="error-text">{{ 'reputation.review.rating_required' | translate }}</span>
              }
            </div>

            <mat-divider></mat-divider>

            <!-- Aspect Ratings -->
            <div class="aspects-section">
              <h3>{{ 'reputation.review.aspects_title' | translate }}</h3>

              <div class="aspect-row">
                <span class="aspect-label">{{ 'reputation.aspect.quality' | translate }}</span>
                <app-rating-stars
                  [value]="aspectQuality"
                  [interactive]="true"
                  (ratingChange)="aspectQuality = $event">
                </app-rating-stars>
              </div>

              <div class="aspect-row">
                <span class="aspect-label">{{ 'reputation.aspect.punctuality' | translate }}</span>
                <app-rating-stars
                  [value]="aspectPunctuality"
                  [interactive]="true"
                  (ratingChange)="aspectPunctuality = $event">
                </app-rating-stars>
              </div>

              <div class="aspect-row">
                <span class="aspect-label">{{ 'reputation.aspect.communication' | translate }}</span>
                <app-rating-stars
                  [value]="aspectCommunication"
                  [interactive]="true"
                  (ratingChange)="aspectCommunication = $event">
                </app-rating-stars>
              </div>

              <div class="aspect-row">
                <span class="aspect-label">{{ 'reputation.aspect.weight_accuracy' | translate }}</span>
                <app-rating-stars
                  [value]="aspectWeight"
                  [interactive]="true"
                  (ratingChange)="aspectWeight = $event">
                </app-rating-stars>
              </div>
            </div>

            <mat-divider></mat-divider>

            <!-- Comment -->
            <mat-form-field appearance="outline" class="full-width">
              <mat-label>{{ 'reputation.review.comment' | translate }}</mat-label>
              <textarea matInput formControlName="comment" rows="4"
                        [placeholder]="'reputation.review.comment_placeholder' | translate">
              </textarea>
              @if (form.get('comment')?.hasError('required')) {
                <mat-error>{{ 'reputation.review.comment_required' | translate }}</mat-error>
              }
              @if (form.get('comment')?.hasError('minlength')) {
                <mat-error>{{ 'reputation.review.comment_min' | translate }}</mat-error>
              }
            </mat-form-field>

            <div class="form-actions">
              <button mat-button type="button" routerLink="..">
                {{ 'common.cancel' | translate }}
              </button>
              <button mat-raised-button color="primary" type="submit"
                      [disabled]="submitting()">
                <mat-icon>send</mat-icon>
                {{ 'reputation.review.submit' | translate }}
              </button>
            </div>
          </form>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .review-container {
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

    .review-form {
      display: flex;
      flex-direction: column;
      gap: 20px;
    }

    .rating-section {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 12px;
      padding: 16px 0;

      label { font-size: 1rem; font-weight: 500; }
    }

    .error-text {
      color: #f44336;
      font-size: 0.8rem;
    }

    .aspects-section {
      padding: 8px 0;

      h3 { margin: 0 0 16px; font-size: 1rem; }
    }

    .aspect-row {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 8px 0;

      .aspect-label { font-size: 0.9rem; color: #444; }
    }

    .full-width { width: 100%; }

    .form-actions {
      display: flex;
      justify-content: flex-end;
      gap: 12px;
    }
  `],
})
export class LeaveReviewComponent {
  private readonly fb = new FormBuilder();
  readonly submitting = signal(false);

  overallRating = 0;
  aspectQuality = 0;
  aspectPunctuality = 0;
  aspectCommunication = 0;
  aspectWeight = 0;
  submitted = false;

  readonly form = this.fb.nonNullable.group({
    comment: ['', [Validators.required, Validators.minLength(10)]],
  });

  constructor(
    private readonly route: ActivatedRoute,
    private readonly router: Router,
  ) {}

  onRatingChange(value: number): void {
    this.overallRating = value;
  }

  onSubmit(): void {
    this.submitted = true;
    if (this.form.invalid || this.overallRating === 0) return;

    this.submitting.set(true);
    const userId = this.route.snapshot.paramMap.get('userId');
    console.log('Review submitted for user:', userId, {
      overallRating: this.overallRating,
      aspects: {
        quality: this.aspectQuality,
        punctuality: this.aspectPunctuality,
        communication: this.aspectCommunication,
        weightAccuracy: this.aspectWeight,
      },
      ...this.form.value,
    });
    // TODO: API call
    this.router.navigate(['/reputation']);
  }
}
