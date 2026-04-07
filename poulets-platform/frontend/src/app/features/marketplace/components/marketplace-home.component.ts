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
import { MatBadgeModule } from '@angular/material/badge';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatDividerModule } from '@angular/material/divider';
import { MatTooltipModule } from '@angular/material/tooltip';
import { TranslateModule } from '@ngx-translate/core';

import { MarketplaceService } from '../services/marketplace.service';
import { Annonce, Besoin, AnnonceFilter, CHICKEN_RACES } from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-marketplace-home',
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
    MatBadgeModule,
    MatProgressSpinnerModule,
    MatDividerModule,
    MatTooltipModule,
    TranslateModule,
  ],
  template: `
    <div class="marketplace-home">
      <!-- Search / Filter Bar -->
      <section class="filter-bar">
        <mat-card>
          <mat-card-content>
            <form [formGroup]="filterForm" (ngSubmit)="applyFilters()" class="filter-form">
              <mat-form-field appearance="outline" class="filter-field">
                <mat-label>{{ 'marketplace.filter.race' | translate }}</mat-label>
                <mat-select formControlName="race">
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
                <input matInput formControlName="location">
              </mat-form-field>

              <mat-form-field appearance="outline" class="filter-field">
                <mat-label>{{ 'marketplace.filter.dateFrom' | translate }}</mat-label>
                <input matInput [matDatepicker]="dateFrom" formControlName="dateFrom">
                <mat-datepicker-toggle matIconSuffix [for]="dateFrom"></mat-datepicker-toggle>
                <mat-datepicker #dateFrom></mat-datepicker>
              </mat-form-field>

              <mat-form-field appearance="outline" class="filter-field">
                <mat-label>{{ 'marketplace.filter.dateTo' | translate }}</mat-label>
                <input matInput [matDatepicker]="dateTo" formControlName="dateTo">
                <mat-datepicker-toggle matIconSuffix [for]="dateTo"></mat-datepicker-toggle>
                <mat-datepicker #dateTo></mat-datepicker>
              </mat-form-field>

              <mat-form-field appearance="outline" class="filter-field">
                <mat-label>{{ 'marketplace.filter.priceMin' | translate }} (FCFA)</mat-label>
                <input matInput type="number" formControlName="priceMin" min="0">
              </mat-form-field>

              <mat-form-field appearance="outline" class="filter-field">
                <mat-label>{{ 'marketplace.filter.priceMax' | translate }} (FCFA)</mat-label>
                <input matInput type="number" formControlName="priceMax" min="0">
              </mat-form-field>

              <div class="filter-actions">
                <button mat-raised-button color="primary" type="submit">
                  <mat-icon>search</mat-icon>
                  {{ 'marketplace.filter.search' | translate }}
                </button>
                <button mat-button type="button" (click)="resetFilters()">
                  <mat-icon>clear</mat-icon>
                  {{ 'marketplace.filter.reset' | translate }}
                </button>
              </div>
            </form>
          </mat-card-content>
        </mat-card>
      </section>

      <!-- Quick Navigation -->
      <section class="quick-nav">
        <a mat-raised-button color="primary" routerLink="annonces">
          <mat-icon>list</mat-icon>
          {{ 'marketplace.nav.allAnnonces' | translate }}
        </a>
        <a mat-raised-button color="accent" routerLink="besoins">
          <mat-icon>shopping_bag</mat-icon>
          {{ 'marketplace.nav.allBesoins' | translate }}
        </a>
        <a mat-raised-button routerLink="matching">
          <mat-icon>compare_arrows</mat-icon>
          {{ 'marketplace.nav.matching' | translate }}
        </a>
        <a mat-stroked-button routerLink="annonces/new">
          <mat-icon>add_circle</mat-icon>
          {{ 'marketplace.nav.publishAnnonce' | translate }}
        </a>
        <a mat-stroked-button routerLink="besoins/new">
          <mat-icon>add_circle_outline</mat-icon>
          {{ 'marketplace.nav.publishBesoin' | translate }}
        </a>
      </section>

      <!-- Split View -->
      <div class="split-view">
        <!-- Left: Latest Annonces -->
        <section class="split-column">
          <div class="column-header">
            <h2>
              <mat-icon>storefront</mat-icon>
              {{ 'marketplace.home.latestAnnonces' | translate }}
            </h2>
            <a mat-button routerLink="annonces" color="primary">
              {{ 'marketplace.home.viewAll' | translate }}
              <mat-icon>arrow_forward</mat-icon>
            </a>
          </div>

          @if (loadingAnnonces()) {
            <div class="loading-container">
              <mat-spinner diameter="40"></mat-spinner>
            </div>
          } @else if (annonces().length === 0) {
            <div class="empty-state">
              <mat-icon>inventory_2</mat-icon>
              <p>{{ 'marketplace.home.noAnnonces' | translate }}</p>
            </div>
          } @else {
            @for (annonce of annonces(); track annonce.id) {
              <mat-card class="annonce-card" [routerLink]="['annonces', annonce.id]">
                <mat-card-header>
                  <mat-icon mat-card-avatar class="race-avatar">egg_alt</mat-icon>
                  <mat-card-title>{{ annonce.race }}</mat-card-title>
                  <mat-card-subtitle>
                    {{ annonce.eleveur.nom }} - {{ annonce.location }}
                  </mat-card-subtitle>
                </mat-card-header>
                <mat-card-content>
                  <div class="annonce-details">
                    <div class="detail-row">
                      <span class="detail-label">{{ 'marketplace.annonce.quantity' | translate }}:</span>
                      <span class="detail-value">{{ annonce.quantity }}</span>
                    </div>
                    <div class="detail-row">
                      <span class="detail-label">{{ 'marketplace.annonce.weight' | translate }}:</span>
                      <span class="detail-value">{{ annonce.currentWeight }} kg</span>
                    </div>
                    <div class="detail-row">
                      <span class="detail-label">{{ 'marketplace.annonce.price' | translate }}:</span>
                      <span class="detail-value price">{{ annonce.pricePerKg | number }} FCFA/kg</span>
                    </div>
                    <div class="detail-row">
                      <span class="detail-label">{{ 'marketplace.annonce.availability' | translate }}:</span>
                      <span class="detail-value">{{ annonce.availabilityStart | date:'shortDate' }}</span>
                    </div>
                  </div>
                  <div class="annonce-badges">
                    @if (annonce.veterinaryStatus === 'VERIFIED') {
                      <mat-icon class="badge-icon verified" matTooltip="{{ 'marketplace.annonce.vetVerified' | translate }}">verified</mat-icon>
                    }
                    @if (annonce.halalCertified) {
                      <mat-icon class="badge-icon halal" matTooltip="{{ 'marketplace.annonce.halalCertified' | translate }}">check_circle</mat-icon>
                    }
                    <span class="rating" matTooltip="{{ 'marketplace.annonce.eleveurRating' | translate }}">
                      <mat-icon class="star-icon">star</mat-icon>
                      {{ annonce.eleveur.note | number:'1.1-1' }}
                    </span>
                  </div>
                </mat-card-content>
              </mat-card>
            }
          }
        </section>

        <!-- Right: Latest Besoins -->
        <section class="split-column">
          <div class="column-header">
            <h2>
              <mat-icon>shopping_bag</mat-icon>
              {{ 'marketplace.home.latestBesoins' | translate }}
            </h2>
            <a mat-button routerLink="besoins" color="primary">
              {{ 'marketplace.home.viewAll' | translate }}
              <mat-icon>arrow_forward</mat-icon>
            </a>
          </div>

          @if (loadingBesoins()) {
            <div class="loading-container">
              <mat-spinner diameter="40"></mat-spinner>
            </div>
          } @else if (besoins().length === 0) {
            <div class="empty-state">
              <mat-icon>search_off</mat-icon>
              <p>{{ 'marketplace.home.noBesoins' | translate }}</p>
            </div>
          } @else {
            @for (besoin of besoins(); track besoin.id) {
              <mat-card class="besoin-card" [routerLink]="['besoins', besoin.id]">
                <mat-card-header>
                  <mat-icon mat-card-avatar class="besoin-avatar">shopping_basket</mat-icon>
                  <mat-card-title>{{ besoin.races.join(', ') }}</mat-card-title>
                  <mat-card-subtitle>
                    {{ besoin.client.nom }} - {{ besoin.location }}
                  </mat-card-subtitle>
                </mat-card-header>
                <mat-card-content>
                  <div class="annonce-details">
                    <div class="detail-row">
                      <span class="detail-label">{{ 'marketplace.besoin.quantity' | translate }}:</span>
                      <span class="detail-value">{{ besoin.quantity }}</span>
                    </div>
                    <div class="detail-row">
                      <span class="detail-label">{{ 'marketplace.besoin.minWeight' | translate }}:</span>
                      <span class="detail-value">{{ besoin.minimumWeight }} kg</span>
                    </div>
                    <div class="detail-row">
                      <span class="detail-label">{{ 'marketplace.besoin.deliveryDate' | translate }}:</span>
                      <span class="detail-value">{{ besoin.deliveryDate | date:'shortDate' }}</span>
                    </div>
                    <div class="detail-row">
                      <span class="detail-label">{{ 'marketplace.besoin.budget' | translate }}:</span>
                      <span class="detail-value price">{{ besoin.maxBudgetPerKg | number }} FCFA/kg</span>
                    </div>
                    <div class="detail-row">
                      <span class="detail-label">{{ 'marketplace.besoin.frequency' | translate }}:</span>
                      <span class="detail-value">{{ 'marketplace.frequency.' + besoin.frequency | translate }}</span>
                    </div>
                  </div>
                  <div class="annonce-badges">
                    @if (besoin.halalRequired) {
                      <mat-icon class="badge-icon halal" matTooltip="{{ 'marketplace.besoin.halalRequired' | translate }}">check_circle</mat-icon>
                    }
                    @if (besoin.veterinaryCertifiedRequired) {
                      <mat-icon class="badge-icon verified" matTooltip="{{ 'marketplace.besoin.vetRequired' | translate }}">verified</mat-icon>
                    }
                  </div>
                </mat-card-content>
              </mat-card>
            }
          }
        </section>
      </div>
    </div>
  `,
  styles: [`
    .marketplace-home {
      padding: 24px;
      max-width: 1400px;
      margin: 0 auto;
    }

    .filter-bar {
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

    .filter-actions {
      display: flex;
      gap: 8px;
      align-items: center;
      padding-top: 4px;
    }

    .quick-nav {
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
      margin-bottom: 24px;
    }

    .split-view {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 24px;
    }

    @media (max-width: 960px) {
      .split-view {
        grid-template-columns: 1fr;
      }
    }

    .split-column {
      display: flex;
      flex-direction: column;
      gap: 16px;
    }

    .column-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    .column-header h2 {
      display: flex;
      align-items: center;
      gap: 8px;
      margin: 0;
      font-size: 1.3rem;
    }

    .annonce-card, .besoin-card {
      cursor: pointer;
      transition: transform 0.15s ease, box-shadow 0.15s ease;
    }

    .annonce-card:hover, .besoin-card:hover {
      transform: translateY(-2px);
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
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

    .besoin-avatar {
      color: #1565c0;
      font-size: 28px;
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .annonce-details {
      margin: 12px 0;
    }

    .detail-row {
      display: flex;
      justify-content: space-between;
      padding: 4px 0;
      border-bottom: 1px solid rgba(0, 0, 0, 0.06);
    }

    .detail-label {
      color: #666;
      font-size: 0.875rem;
    }

    .detail-value {
      font-weight: 500;
    }

    .detail-value.price {
      color: #2e7d32;
      font-weight: 600;
    }

    .annonce-badges {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-top: 8px;
    }

    .badge-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    .badge-icon.verified {
      color: #4caf50;
    }

    .badge-icon.halal {
      color: #1565c0;
    }

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
      padding: 40px;
    }

    .empty-state {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 40px;
      color: #999;
    }

    .empty-state mat-icon {
      font-size: 48px;
      width: 48px;
      height: 48px;
      margin-bottom: 12px;
    }
  `],
})
export class MarketplaceHomeComponent implements OnInit {
  private readonly marketplace = inject(MarketplaceService);
  private readonly fb = inject(FormBuilder);

  readonly races = CHICKEN_RACES;
  readonly annonces = signal<Annonce[]>([]);
  readonly besoins = signal<Besoin[]>([]);
  readonly loadingAnnonces = signal(true);
  readonly loadingBesoins = signal(true);

  readonly filterForm: FormGroup = this.fb.group({
    race: [''],
    weightMin: [null],
    weightMax: [null],
    location: [''],
    dateFrom: [null],
    dateTo: [null],
    priceMin: [null],
    priceMax: [null],
  });

  ngOnInit(): void {
    this.loadAnnonces();
    this.loadBesoins();
  }

  applyFilters(): void {
    this.loadAnnonces();
  }

  resetFilters(): void {
    this.filterForm.reset();
    this.loadAnnonces();
  }

  private loadAnnonces(): void {
    this.loadingAnnonces.set(true);
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

    this.marketplace.getAnnonces(filter, 0, 8).subscribe({
      next: (page) => {
        this.annonces.set(page.content);
        this.loadingAnnonces.set(false);
      },
      error: () => this.loadingAnnonces.set(false),
    });
  }

  private loadBesoins(): void {
    this.loadingBesoins.set(true);
    this.marketplace.getBesoins(undefined, 0, 8).subscribe({
      next: (page) => {
        this.besoins.set(page.content);
        this.loadingBesoins.set(false);
      },
      error: () => this.loadingBesoins.set(false),
    });
  }
}
