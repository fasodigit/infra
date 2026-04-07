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
import { MatChipsModule } from '@angular/material/chips';
import { MatPaginatorModule, PageEvent } from '@angular/material/paginator';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatTooltipModule } from '@angular/material/tooltip';
import { TranslateModule } from '@ngx-translate/core';

import { MarketplaceService } from '../services/marketplace.service';
import {
  Besoin,
  BesoinFilter,
  BesoinFrequency,
  CHICKEN_RACES,
} from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-besoins-list',
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
    MatChipsModule,
    MatPaginatorModule,
    MatProgressSpinnerModule,
    MatTooltipModule,
    TranslateModule,
  ],
  template: `
    <div class="besoins-page">
      <div class="page-header">
        <h1>
          <mat-icon>shopping_bag</mat-icon>
          {{ 'marketplace.besoins.title' | translate }}
        </h1>
        <a mat-raised-button color="primary" routerLink="/marketplace/besoins/new">
          <mat-icon>add</mat-icon>
          {{ 'marketplace.besoins.publish' | translate }}
        </a>
      </div>

      <!-- Filters -->
      <mat-card class="filter-card">
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
              <mat-label>{{ 'marketplace.filter.quantityMin' | translate }}</mat-label>
              <input matInput type="number" formControlName="quantityMin" min="0">
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.budgetMin' | translate }} (FCFA)</mat-label>
              <input matInput type="number" formControlName="budgetMin" min="0">
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.budgetMax' | translate }} (FCFA)</mat-label>
              <input matInput type="number" formControlName="budgetMax" min="0">
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.location' | translate }}</mat-label>
              <input matInput formControlName="location">
            </mat-form-field>

            <mat-form-field appearance="outline" class="filter-field">
              <mat-label>{{ 'marketplace.filter.frequency' | translate }}</mat-label>
              <mat-select formControlName="frequency">
                <mat-option value="">{{ 'marketplace.filter.allFrequencies' | translate }}</mat-option>
                @for (freq of frequencies; track freq) {
                  <mat-option [value]="freq">{{ 'marketplace.frequency.' + freq | translate }}</mat-option>
                }
              </mat-select>
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

      <!-- Results -->
      @if (loading()) {
        <div class="loading-container">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else if (besoins().length === 0) {
        <div class="empty-state">
          <mat-icon>search_off</mat-icon>
          <p>{{ 'marketplace.besoins.empty' | translate }}</p>
          <a mat-raised-button color="primary" routerLink="/marketplace/besoins/new">
            {{ 'marketplace.besoins.publishFirst' | translate }}
          </a>
        </div>
      } @else {
        <div class="besoins-grid">
          @for (besoin of besoins(); track besoin.id) {
            <mat-card class="besoin-card" [routerLink]="['/marketplace/besoins', besoin.id]">
              <mat-card-header>
                <mat-icon mat-card-avatar class="besoin-avatar">shopping_basket</mat-icon>
                <mat-card-title>{{ besoin.races.join(', ') }}</mat-card-title>
                <mat-card-subtitle>{{ besoin.client.nom }} - {{ besoin.location }}</mat-card-subtitle>
              </mat-card-header>

              <mat-card-content>
                <div class="card-details">
                  <div class="detail-row">
                    <span class="label">{{ 'marketplace.besoin.quantity' | translate }}</span>
                    <span class="value">{{ besoin.quantity }}</span>
                  </div>
                  <div class="detail-row">
                    <span class="label">{{ 'marketplace.besoin.minWeight' | translate }}</span>
                    <span class="value">{{ besoin.minimumWeight | number:'1.1-1' }} kg</span>
                  </div>
                  <div class="detail-row">
                    <span class="label">{{ 'marketplace.besoin.deliveryDate' | translate }}</span>
                    <span class="value">{{ besoin.deliveryDate | date:'shortDate' }}</span>
                  </div>
                  <div class="detail-row">
                    <span class="label">{{ 'marketplace.besoin.budget' | translate }}</span>
                    <span class="value price">{{ besoin.maxBudgetPerKg | number }} FCFA/kg</span>
                  </div>
                </div>

                <div class="card-badges">
                  <mat-chip-set>
                    <mat-chip [class]="'freq-' + besoin.frequency.toLowerCase()">
                      {{ 'marketplace.frequency.' + besoin.frequency | translate }}
                    </mat-chip>
                    @if (besoin.halalRequired) {
                      <mat-chip class="badge-halal"
                        matTooltip="{{ 'marketplace.besoin.halalRequired' | translate }}">
                        <mat-icon>check_circle</mat-icon>
                        {{ 'marketplace.besoin.halal' | translate }}
                      </mat-chip>
                    }
                    @if (besoin.veterinaryCertifiedRequired) {
                      <mat-chip class="badge-vet"
                        matTooltip="{{ 'marketplace.besoin.vetRequired' | translate }}">
                        <mat-icon>verified</mat-icon>
                        {{ 'marketplace.besoin.vet' | translate }}
                      </mat-chip>
                    }
                  </mat-chip-set>
                </div>

                @if (besoin.specialNotes) {
                  <p class="special-notes">
                    <mat-icon>note</mat-icon>
                    {{ besoin.specialNotes }}
                  </p>
                }
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
    .besoins-page {
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

    .filter-actions {
      display: flex;
      gap: 8px;
      align-items: center;
      padding-top: 4px;
    }

    .besoins-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
      gap: 20px;
      margin-bottom: 24px;
    }

    .besoin-card {
      cursor: pointer;
      transition: transform 0.15s ease, box-shadow 0.15s ease;
    }

    .besoin-card:hover {
      transform: translateY(-3px);
      box-shadow: 0 6px 16px rgba(0, 0, 0, 0.15);
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
    }

    .value.price {
      color: #2e7d32;
      font-weight: 600;
    }

    .card-badges {
      margin: 12px 0 8px;
    }

    .freq-ponctuel { --mdc-chip-elevated-container-color: #e8eaf6; }
    .freq-hebdomadaire { --mdc-chip-elevated-container-color: #e3f2fd; }
    .freq-bi_mensuel { --mdc-chip-elevated-container-color: #e0f2f1; }
    .freq-mensuel { --mdc-chip-elevated-container-color: #f3e5f5; }
    .badge-halal { --mdc-chip-elevated-container-color: #e3f2fd; }
    .badge-vet { --mdc-chip-elevated-container-color: #e8f5e9; }

    .special-notes {
      display: flex;
      align-items: flex-start;
      gap: 6px;
      padding: 8px;
      background: #fffde7;
      border-radius: 6px;
      font-size: 0.85rem;
      color: #666;
      margin-top: 8px;
    }

    .special-notes mat-icon {
      font-size: 18px;
      width: 18px;
      height: 18px;
      color: #ffc107;
      flex-shrink: 0;
      margin-top: 2px;
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
export class BesoinsListComponent implements OnInit {
  private readonly marketplace = inject(MarketplaceService);
  private readonly fb = inject(FormBuilder);

  readonly races = CHICKEN_RACES;
  readonly frequencies: BesoinFrequency[] = ['PONCTUEL', 'HEBDOMADAIRE', 'BI_MENSUEL', 'MENSUEL'];
  readonly besoins = signal<Besoin[]>([]);
  readonly loading = signal(true);
  readonly totalElements = signal(0);
  readonly currentPage = signal(0);
  readonly pageSize = 12;

  readonly filterForm: FormGroup = this.fb.group({
    race: [''],
    quantityMin: [null],
    budgetMin: [null],
    budgetMax: [null],
    location: [''],
    frequency: [''],
  });

  ngOnInit(): void {
    this.loadBesoins();
  }

  applyFilters(): void {
    this.currentPage.set(0);
    this.loadBesoins();
  }

  resetFilters(): void {
    this.filterForm.reset();
    this.currentPage.set(0);
    this.loadBesoins();
  }

  onPageChange(event: PageEvent): void {
    this.currentPage.set(event.pageIndex);
    this.loadBesoins();
  }

  private loadBesoins(): void {
    this.loading.set(true);
    const v = this.filterForm.value;
    const filter: BesoinFilter = {};
    if (v.race) filter.race = v.race;
    if (v.quantityMin != null) filter.quantityMin = v.quantityMin;
    if (v.budgetMin != null) filter.budgetMin = v.budgetMin;
    if (v.budgetMax != null) filter.budgetMax = v.budgetMax;
    if (v.location) filter.location = v.location;
    if (v.frequency) filter.frequency = v.frequency;

    this.marketplace.getBesoins(filter, this.currentPage(), this.pageSize).subscribe({
      next: (page) => {
        this.besoins.set(page.content);
        this.totalElements.set(page.totalElements);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }
}
