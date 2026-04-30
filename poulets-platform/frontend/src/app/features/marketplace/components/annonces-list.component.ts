import { Component, OnInit, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { ReactiveFormsModule, FormBuilder, FormGroup } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatNativeDateModule } from '@angular/material/core';
import { MatChipsModule } from '@angular/material/chips';
import { MatPaginatorModule, PageEvent } from '@angular/material/paginator';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { TranslateModule } from '@ngx-translate/core';

import { MarketplaceService } from '../services/marketplace.service';
import { Annonce, AnnonceFilter, CHICKEN_RACES } from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-annonces-list',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    ReactiveFormsModule,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    MatDatepickerModule,
    MatNativeDateModule,
    MatChipsModule,
    MatPaginatorModule,
    MatProgressSpinnerModule,
    MatTooltipModule,
    MatCheckboxModule,
    TranslateModule,
  ],
  template: `
    <div class="annonces-page" data-testid="annonces-page">
      <div class="page-header">
        <h1>
          <mat-icon>storefront</mat-icon>
          {{ 'marketplace.annonces.title' | translate }}
        </h1>
        <a mat-raised-button color="primary" routerLink="/marketplace/annonces/new"
           data-testid="annonces-action-publish">
          <mat-icon>add</mat-icon>
          {{ 'marketplace.annonces.publish' | translate }}
        </a>
      </div>

      <!-- Filters -->
      <mat-card class="filter-card">
        <mat-card-content>
          <form [formGroup]="filterForm" (ngSubmit)="applyFilters()" class="filter-form"
                data-testid="annonces-form-filter">
            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.race' | translate }}</mat-label>
              <mat-select formControlName="race" data-testid="annonces-filter-race">
                <mat-option value="">{{ 'marketplace.filter.allRaces' | translate }}</mat-option>
                @for (race of races; track race) {
                  <mat-option [value]="race">{{ race }}</mat-option>
                }
              </mat-select>
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.weightMin' | translate }} (kg)</mat-label>
              <input matInput type="number" formControlName="weightMin" min="0" step="0.1">
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.weightMax' | translate }} (kg)</mat-label>
              <input matInput type="number" formControlName="weightMax" min="0" step="0.1">
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.location' | translate }}</mat-label>
              <input matInput formControlName="location" data-testid="annonces-filter-location">
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.dateFrom' | translate }}</mat-label>
              <input matInput [matDatepicker]="pickerFrom" formControlName="dateFrom">
              <mat-datepicker-toggle matIconSuffix [for]="pickerFrom"></mat-datepicker-toggle>
              <mat-datepicker #pickerFrom></mat-datepicker>
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.dateTo' | translate }}</mat-label>
              <input matInput [matDatepicker]="pickerTo" formControlName="dateTo">
              <mat-datepicker-toggle matIconSuffix [for]="pickerTo"></mat-datepicker-toggle>
              <mat-datepicker #pickerTo></mat-datepicker>
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.priceMin' | translate }} (FCFA)</mat-label>
              <input matInput type="number" formControlName="priceMin" min="0">
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.priceMax' | translate }} (FCFA)</mat-label>
              <input matInput type="number" formControlName="priceMax" min="0">
            </mat-form-field>

            <div class="filter-checkboxes">
              <mat-checkbox formControlName="halalOnly">
                {{ 'marketplace.filter.halalOnly' | translate }}
              </mat-checkbox>
              <mat-checkbox formControlName="veterinaryVerified">
                {{ 'marketplace.filter.vetVerified' | translate }}
              </mat-checkbox>
            </div>

            <div class="filter-actions">
              <button mat-raised-button color="primary" type="submit"
                      data-testid="annonces-form-submit">
                <mat-icon>search</mat-icon>
                {{ 'marketplace.filter.search' | translate }}
              </button>
              <button mat-button type="button" (click)="resetFilters()"
                      data-testid="annonces-filter-clear">
                <mat-icon>clear</mat-icon>
                {{ 'marketplace.filter.reset' | translate }}
              </button>
            </div>
          </form>
        </mat-card-content>
      </mat-card>

      <!-- Results -->
      @if (loading()) {
        <div class="loading-container">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else if (annonces().length === 0) {
        <div class="empty-state" data-testid="annonces-empty">
          <mat-icon>inventory_2</mat-icon>
          <p>{{ 'marketplace.annonces.empty' | translate }}</p>
          <a mat-raised-button color="primary" routerLink="/marketplace/annonces/new">
            {{ 'marketplace.annonces.publishFirst' | translate }}
          </a>
        </div>
      } @else {
        <div class="annonces-grid" data-testid="annonces-list">
          @for (annonce of annonces(); track annonce.id) {
            <mat-card class="annonce-card" [routerLink]="['/marketplace/annonces', annonce.id]"
                      [attr.data-testid]="'annonces-list-item-' + annonce.id">
              @if (annonce.photos && annonce.photos.length > 0) {
                <img mat-card-image [src]="annonce.photos[0]" [alt]="annonce.race" class="card-image">
              } @else {
                <div class="card-image-placeholder">
                  <mat-icon>egg_alt</mat-icon>
                </div>
              }

              <mat-card-header>
                <mat-icon mat-card-avatar class="race-avatar">egg_alt</mat-icon>
                <mat-card-title>{{ annonce.race }}</mat-card-title>
                <mat-card-subtitle>
                  {{ annonce.eleveur.nom }} - {{ annonce.location }}
                </mat-card-subtitle>
              </mat-card-header>

              <mat-card-content>
                <div class="card-details">
                  <div class="detail-row">
                    <span class="label">{{ 'marketplace.annonce.quantity' | translate }}</span>
                    <span class="value">{{ annonce.quantity }}</span>
                  </div>
                  <div class="detail-row">
                    <span class="label">{{ 'marketplace.annonce.weight' | translate }}</span>
                    <span class="value">{{ annonce.currentWeight | number:'1.1-1' }} - {{ annonce.estimatedWeight | number:'1.1-1' }} kg</span>
                  </div>
                  <div class="detail-row">
                    <span class="label">{{ 'marketplace.annonce.pricePerKg' | translate }}</span>
                    <span class="value price">{{ annonce.pricePerKg | number }} FCFA/kg</span>
                  </div>
                  <div class="detail-row">
                    <span class="label">{{ 'marketplace.annonce.availability' | translate }}</span>
                    <span class="value">{{ annonce.availabilityStart | date:'shortDate' }} - {{ annonce.availabilityEnd | date:'shortDate' }}</span>
                  </div>
                </div>

                <div class="card-badges">
                  @if (annonce.veterinaryStatus === 'VERIFIED') {
                    <mat-icon class="badge verified"
                      matTooltip="{{ 'marketplace.annonce.vetVerified' | translate }}">verified</mat-icon>
                  } @else if (annonce.veterinaryStatus === 'PENDING') {
                    <mat-icon class="badge pending"
                      matTooltip="{{ 'marketplace.annonce.vetPending' | translate }}">pending</mat-icon>
                  }
                  @if (annonce.halalCertified) {
                    <mat-icon class="badge halal"
                      matTooltip="{{ 'marketplace.annonce.halalCertified' | translate }}">check_circle</mat-icon>
                  }
                  <span class="rating" matTooltip="{{ 'marketplace.annonce.eleveurRating' | translate }}">
                    <mat-icon class="star-icon">star</mat-icon>
                    {{ annonce.eleveur.note | number:'1.1-1' }}
                  </span>
                </div>
              </mat-card-content>
            </mat-card>
          }
        </div>

        <mat-paginator
          [length]="totalElements()"
          [pageSize]="pageSize"
          [pageIndex]="currentPage()"
          [pageSizeOptions]="[12, 24, 48]"
          (page)="onPageChange($event)"
          showFirstLastButtons>
        </mat-paginator>
      }
    </div>
  `,
  styles: [`
    .annonces-page {
      padding: 24px;
      max-width: 1400px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 24px;
    }

    .page-header h1 {
      display: flex;
      align-items: center;
      gap: 8px;
      margin: 0;
    }

    .filter-card {
      margin-bottom: 24px;
    }

    .filter-form {
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
      align-items: flex-start;
    }

    .filter-field {
      flex: 1 1 180px;
      min-width: 150px;
    }

    .filter-checkboxes {
      display: flex;
      gap: 16px;
      align-items: center;
      padding-top: 8px;
    }

    .filter-actions {
      display: flex;
      gap: 8px;
      align-items: center;
      padding-top: 4px;
    }

    .annonces-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
      gap: 20px;
      margin-bottom: 24px;
    }

    .annonce-card {
      cursor: pointer;
      transition: transform 0.15s ease, box-shadow 0.15s ease;
    }

    .annonce-card:hover {
      transform: translateY(-3px);
      box-shadow: 0 6px 16px rgba(0, 0, 0, 0.15);
    }

    .card-image {
      height: 180px;
      object-fit: cover;
    }

    .card-image-placeholder {
      height: 120px;
      display: flex;
      align-items: center;
      justify-content: center;
      background: linear-gradient(135deg, #e8f5e9, #c8e6c9);
    }

    .card-image-placeholder mat-icon {
      font-size: 48px;
      width: 48px;
      height: 48px;
      color: #2e7d32;
    }

    .race-avatar {
      color: #2e7d32;
      font-size: 28px;
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .card-details {
      margin: 12px 0;
    }

    .detail-row {
      display: flex;
      justify-content: space-between;
      padding: 4px 0;
      border-bottom: 1px solid rgba(0, 0, 0, 0.06);
    }

    .label {
      color: #666;
      font-size: 0.85rem;
    }

    .value {
      font-weight: 500;
      font-size: 0.9rem;
    }

    .value.price {
      color: #2e7d32;
      font-weight: 600;
    }

    .card-badges {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-top: 8px;
    }

    .badge {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    .badge.verified { color: #4caf50; }
    .badge.pending { color: #ff9800; }
    .badge.halal { color: #1565c0; }

    .rating {
      display: flex;
      align-items: center;
      gap: 2px;
      margin-left: auto;
      color: #ff9800;
      font-weight: 500;
    }

    .star-icon {
      font-size: 18px;
      width: 18px;
      height: 18px;
      color: #ff9800;
    }

    .loading-container {
      display: flex;
      justify-content: center;
      padding: 60px;
    }

    .empty-state {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 60px;
      color: #999;
    }

    .empty-state mat-icon {
      font-size: 64px;
      width: 64px;
      height: 64px;
      margin-bottom: 16px;
    }

    .empty-state p {
      margin-bottom: 16px;
      font-size: 1.1rem;
    }
  `],
})
export class AnnoncesListComponent implements OnInit {
  private readonly marketplace = inject(MarketplaceService);
  private readonly fb = inject(FormBuilder);

  readonly races = CHICKEN_RACES;
  readonly annonces = signal<Annonce[]>([]);
  readonly loading = signal(true);
  readonly totalElements = signal(0);
  readonly currentPage = signal(0);
  readonly pageSize = 12;

  readonly filterForm: FormGroup = this.fb.group({
    race: [''],
    weightMin: [null],
    weightMax: [null],
    location: [''],
    dateFrom: [null],
    dateTo: [null],
    priceMin: [null],
    priceMax: [null],
    halalOnly: [false],
    veterinaryVerified: [false],
  });

  ngOnInit(): void {
    this.loadAnnonces();
  }

  applyFilters(): void {
    this.currentPage.set(0);
    this.loadAnnonces();
  }

  resetFilters(): void {
    this.filterForm.reset({ halalOnly: false, veterinaryVerified: false });
    this.currentPage.set(0);
    this.loadAnnonces();
  }

  onPageChange(event: PageEvent): void {
    this.currentPage.set(event.pageIndex);
    this.loadAnnonces();
  }

  private loadAnnonces(): void {
    this.loading.set(true);
    const v = this.filterForm.value;
    const filter: AnnonceFilter = {};
    if (v.race) filter.race = v.race;
    if (v.weightMin != null) filter.weightMin = v.weightMin;
    if (v.weightMax != null) filter.weightMax = v.weightMax;
    if (v.location) filter.location = v.location;
    if (v.dateFrom) filter.dateFrom = new Date(v.dateFrom).toISOString();
    if (v.dateTo) filter.dateTo = new Date(v.dateTo).toISOString();
    if (v.priceMin != null) filter.priceMin = v.priceMin;
    if (v.priceMax != null) filter.priceMax = v.priceMax;
    if (v.halalOnly) filter.halalOnly = true;
    if (v.veterinaryVerified) filter.veterinaryVerified = true;

    this.marketplace.getAnnonces(filter, this.currentPage(), this.pageSize).subscribe({
      next: (page) => {
        this.annonces.set(page.content);
        this.totalElements.set(page.totalElements);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }
}
