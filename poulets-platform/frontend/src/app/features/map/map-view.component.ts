import { Component, OnInit, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatSelectModule } from '@angular/material/select';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatSliderModule } from '@angular/material/slider';
import { MatListModule } from '@angular/material/list';
import { MatChipsModule } from '@angular/material/chips';
import { TranslateModule } from '@ngx-translate/core';
import { DomSanitizer, SafeResourceUrl } from '@angular/platform-browser';
import { RatingStarsComponent } from '@shared/components/rating-stars/rating-stars.component';

interface NearbyActor {
  id: string;
  name: string;
  role: string;
  distance: number;
  rating: number;
  specialties: string[];
}

@Component({
  selector: 'app-map-view',
  standalone: true,
  imports: [
    CommonModule,
    FormsModule,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatSelectModule,
    MatFormFieldModule,
    MatSliderModule,
    MatListModule,
    MatChipsModule,
    TranslateModule,
    RatingStarsComponent,
  ],
  template: `
    <div class="map-container">
      <div class="page-header">
        <h1>{{ 'map.title' | translate }}</h1>
      </div>

      <!-- Filters -->
      <mat-card class="filters-card">
        <mat-card-content>
          <div class="filters-row">
            <mat-form-field appearance="outline">
              <mat-label>{{ 'map.filter.role' | translate }}</mat-label>
              <mat-select [(value)]="selectedRole" (selectionChange)="applyFilters()">
                <mat-option value="all">{{ 'map.filter.all_roles' | translate }}</mat-option>
                <mat-option value="eleveur">{{ 'map.filter.eleveur' | translate }}</mat-option>
                <mat-option value="client">{{ 'map.filter.client' | translate }}</mat-option>
                <mat-option value="producteur_aliment">{{ 'map.filter.producteur' | translate }}</mat-option>
              </mat-select>
            </mat-form-field>

            <mat-form-field appearance="outline">
              <mat-label>{{ 'map.filter.race' | translate }}</mat-label>
              <mat-select [(value)]="selectedRace" (selectionChange)="applyFilters()">
                <mat-option value="all">{{ 'map.filter.all_races' | translate }}</mat-option>
                <mat-option value="bicyclette">Poulet bicyclette</mat-option>
                <mat-option value="chair">Poulet de chair</mat-option>
                <mat-option value="pintade">Pintade</mat-option>
                <mat-option value="dinde">Dinde</mat-option>
              </mat-select>
            </mat-form-field>

            <div class="radius-control">
              <label>{{ 'map.filter.radius' | translate }}: {{ radius }}km</label>
              <mat-slider min="5" max="100" step="5" discrete>
                <input matSliderThumb [(ngModel)]="radius" (valueChange)="applyFilters()">
              </mat-slider>
            </div>
          </div>
        </mat-card-content>
      </mat-card>

      <!-- Map -->
      <mat-card class="map-card">
        <mat-card-content>
          <div class="map-frame">
            <iframe
              [src]="mapUrl"
              width="100%"
              height="400"
              frameborder="0"
              allowfullscreen
              loading="lazy"
              referrerpolicy="no-referrer-when-downgrade">
            </iframe>
          </div>
        </mat-card-content>
      </mat-card>

      <!-- Nearby Actors -->
      <mat-card>
        <mat-card-header>
          <mat-card-title>
            {{ 'map.nearby.title' | translate }} ({{ filteredActors().length }})
          </mat-card-title>
        </mat-card-header>
        <mat-card-content>
          @if (filteredActors().length > 0) {
            <mat-list>
              @for (actor of filteredActors(); track actor.id) {
                <mat-list-item class="actor-item">
                  <div class="actor-content">
                    <div class="actor-avatar" [class]="'role-' + actor.role">
                      <mat-icon>{{ getRoleIcon(actor.role) }}</mat-icon>
                    </div>
                    <div class="actor-info">
                      <span class="actor-name">{{ actor.name }}</span>
                      <span class="actor-role">{{ actor.role }}</span>
                      <div class="actor-specialties">
                        @for (spec of actor.specialties; track spec) {
                          <mat-chip class="specialty-chip">{{ spec }}</mat-chip>
                        }
                      </div>
                    </div>
                    <div class="actor-meta">
                      <app-rating-stars [value]="actor.rating" [showValue]="true"></app-rating-stars>
                      <span class="actor-distance">
                        <mat-icon>place</mat-icon>
                        {{ actor.distance }} km
                      </span>
                    </div>
                  </div>
                </mat-list-item>
              }
            </mat-list>
          } @else {
            <div class="empty-state">
              <mat-icon>location_off</mat-icon>
              <p>{{ 'map.nearby.empty' | translate }}</p>
            </div>
          }
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .map-container {
      padding: 24px;
      max-width: 1100px;
      margin: 0 auto;
    }

    .page-header {
      margin-bottom: 24px;
      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .filters-card { margin-bottom: 16px; }

    .filters-row {
      display: flex;
      gap: 16px;
      align-items: center;
      flex-wrap: wrap;

      mat-form-field { flex: 1; min-width: 180px; }
    }

    .radius-control {
      display: flex;
      flex-direction: column;
      min-width: 180px;

      label { font-size: 0.85rem; color: #666; margin-bottom: 4px; }
    }

    .map-card {
      margin-bottom: 24px;
    }

    .map-frame {
      border-radius: 8px;
      overflow: hidden;

      iframe { display: block; border-radius: 8px; }
    }

    .actor-item {
      height: auto !important;
      padding: 16px 0 !important;
      border-bottom: 1px solid #f0f0f0;
    }

    .actor-content {
      display: flex;
      align-items: center;
      gap: 16px;
      width: 100%;
    }

    .actor-avatar {
      width: 48px;
      height: 48px;
      border-radius: 50%;
      display: flex;
      align-items: center;
      justify-content: center;
      color: white;

      &.role-eleveur { background: #4caf50; }
      &.role-client { background: #2196f3; }
      &.role-producteur_aliment { background: #ff9800; }
    }

    .actor-info {
      flex: 1;

      .actor-name { font-weight: 600; display: block; }
      .actor-role { font-size: 0.8rem; color: #666; display: block; text-transform: capitalize; }
    }

    .actor-specialties {
      display: flex;
      gap: 4px;
      margin-top: 4px;
      flex-wrap: wrap;
    }

    .specialty-chip {
      font-size: 0.7rem !important;
      min-height: 24px !important;
    }

    .actor-meta {
      display: flex;
      flex-direction: column;
      align-items: flex-end;
      gap: 4px;
    }

    .actor-distance {
      display: flex;
      align-items: center;
      gap: 4px;
      font-size: 0.8rem;
      color: #999;

      mat-icon { font-size: 16px; width: 16px; height: 16px; }
    }

    .empty-state {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 32px;
      color: #999;

      mat-icon { font-size: 48px; width: 48px; height: 48px; margin-bottom: 16px; }
    }
  `],
})
export class MapViewComponent implements OnInit {
  readonly filteredActors = signal<NearbyActor[]>([]);
  private allActors: NearbyActor[] = [];

  selectedRole = 'all';
  selectedRace = 'all';
  radius = 25;

  mapUrl: SafeResourceUrl;

  constructor(private readonly sanitizer: DomSanitizer) {
    // Ouagadougou center coordinates
    this.mapUrl = this.sanitizer.bypassSecurityTrustResourceUrl(
      'https://www.openstreetmap.org/export/embed.html?bbox=-1.6,12.3,-1.4,12.4&layer=mapnik&marker=12.3657,-1.5339'
    );
  }

  ngOnInit(): void {
    this.loadNearbyActors();
  }

  applyFilters(): void {
    let actors = this.allActors;
    if (this.selectedRole !== 'all') {
      actors = actors.filter(a => a.role === this.selectedRole);
    }
    if (this.selectedRace !== 'all') {
      actors = actors.filter(a =>
        a.specialties.some(s => s.toLowerCase().includes(this.selectedRace))
      );
    }
    actors = actors.filter(a => a.distance <= this.radius);
    this.filteredActors.set(actors);
  }

  getRoleIcon(role: string): string {
    switch (role) {
      case 'eleveur': return 'agriculture';
      case 'client': return 'restaurant';
      case 'producteur_aliment': return 'factory';
      default: return 'person';
    }
  }

  private loadNearbyActors(): void {
    this.allActors = [
      { id: 'a1', name: 'Ferme Ouedraogo', role: 'eleveur', distance: 5, rating: 4.5, specialties: ['Poulet bicyclette', 'Pintade'] },
      { id: 'a2', name: 'Ferme Kabore & Fils', role: 'eleveur', distance: 12, rating: 4.2, specialties: ['Poulet de chair', 'Dinde'] },
      { id: 'a3', name: 'Restaurant Le Sahel', role: 'client', distance: 8, rating: 4.8, specialties: ['Poulet bicyclette'] },
      { id: 'a4', name: 'Aliments du Faso', role: 'producteur_aliment', distance: 15, rating: 4.0, specialties: ['Demarrage', 'Croissance', 'Finition'] },
      { id: 'a5', name: 'Hotel Splendide', role: 'client', distance: 10, rating: 4.6, specialties: ['Poulet de chair', 'Pintade'] },
      { id: 'a6', name: 'Groupement Eleveurs Koudougou', role: 'eleveur', distance: 22, rating: 4.3, specialties: ['Poulet bicyclette', 'Poulet fermier'] },
    ];
    this.filteredActors.set(this.allActors);
  }
}
