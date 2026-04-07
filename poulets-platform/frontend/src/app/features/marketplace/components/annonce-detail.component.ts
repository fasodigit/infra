import { Component, OnInit, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatDividerModule } from '@angular/material/divider';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatTabsModule } from '@angular/material/tabs';
import { TranslateModule } from '@ngx-translate/core';

import { MarketplaceService } from '../services/marketplace.service';
import { Annonce } from '../../../shared/models/marketplace.models';

@Component({
  selector: 'app-annonce-detail',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    MatDividerModule,
    MatProgressSpinnerModule,
    MatTooltipModule,
    MatTabsModule,
    TranslateModule,
  ],
  template: `
    <div class="annonce-detail-page">
      @if (loading()) {
        <div class="loading-container">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else if (annonce(); as a) {
        <!-- Breadcrumb -->
        <div class="breadcrumb">
          <a routerLink="/marketplace">{{ 'marketplace.title' | translate }}</a>
          <mat-icon>chevron_right</mat-icon>
          <a routerLink="/marketplace/annonces">{{ 'marketplace.annonces.title' | translate }}</a>
          <mat-icon>chevron_right</mat-icon>
          <span>{{ a.race }}</span>
        </div>

        <div class="detail-layout">
          <!-- Main Content -->
          <div class="main-content">
            <!-- Photo Gallery -->
            @if (a.photos && a.photos.length > 0) {
              <mat-card class="photo-card">
                <div class="photo-gallery">
                  <img [src]="a.photos[selectedPhoto()]" [alt]="a.race" class="main-photo">
                  @if (a.photos.length > 1) {
                    <div class="photo-thumbnails">
                      @for (photo of a.photos; track $index) {
                        <img [src]="photo"
                          [class.active]="$index === selectedPhoto()"
                          (click)="selectedPhoto.set($index)"
                          alt="Photo {{ $index + 1 }}">
                      }
                    </div>
                  }
                </div>
              </mat-card>
            }

            <!-- Details -->
            <mat-card class="details-card">
              <mat-card-header>
                <mat-card-title>
                  <h2>{{ a.race }} - {{ a.quantity }} {{ 'marketplace.annonce.units' | translate }}</h2>
                </mat-card-title>
              </mat-card-header>
              <mat-card-content>
                <!-- Badges -->
                <div class="status-badges">
                  <mat-chip-set>
                    <mat-chip [class]="'status-' + a.status.toLowerCase()">
                      {{ 'marketplace.annonce.status.' + a.status | translate }}
                    </mat-chip>
                    @if (a.veterinaryStatus === 'VERIFIED') {
                      <mat-chip class="badge-verified">
                        <mat-icon>verified</mat-icon>
                        {{ 'marketplace.annonce.vetVerified' | translate }}
                      </mat-chip>
                    } @else if (a.veterinaryStatus === 'PENDING') {
                      <mat-chip class="badge-pending">
                        <mat-icon>pending</mat-icon>
                        {{ 'marketplace.annonce.vetPending' | translate }}
                      </mat-chip>
                    } @else {
                      <mat-chip class="badge-none">
                        {{ 'marketplace.annonce.vetNotProvided' | translate }}
                      </mat-chip>
                    }
                    @if (a.halalCertified) {
                      <mat-chip class="badge-halal">
                        <mat-icon>check_circle</mat-icon>
                        {{ 'marketplace.annonce.halalCertified' | translate }}
                      </mat-chip>
                    }
                    @if (a.isGroupement) {
                      <mat-chip>
                        <mat-icon>groups</mat-icon>
                        {{ 'marketplace.annonce.groupement' | translate }}
                      </mat-chip>
                    }
                  </mat-chip-set>
                </div>

                <mat-divider></mat-divider>

                <!-- Key Info Grid -->
                <div class="info-grid">
                  <div class="info-item">
                    <mat-icon>inventory_2</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.annonce.quantity' | translate }}</span>
                      <span class="info-value">{{ a.quantity }}</span>
                    </div>
                  </div>
                  <div class="info-item">
                    <mat-icon>monitor_weight</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.annonce.currentWeight' | translate }}</span>
                      <span class="info-value">{{ a.currentWeight | number:'1.1-1' }} kg</span>
                    </div>
                  </div>
                  <div class="info-item">
                    <mat-icon>trending_up</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.annonce.estimatedWeight' | translate }}</span>
                      <span class="info-value">{{ a.estimatedWeight | number:'1.1-1' }} kg</span>
                    </div>
                  </div>
                  <div class="info-item">
                    <mat-icon>event</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.annonce.targetDate' | translate }}</span>
                      <span class="info-value">{{ a.targetDate | date:'mediumDate' }}</span>
                    </div>
                  </div>
                  <div class="info-item highlight">
                    <mat-icon>payments</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.annonce.pricePerKg' | translate }}</span>
                      <span class="info-value price">{{ a.pricePerKg | number }} FCFA/kg</span>
                    </div>
                  </div>
                  <div class="info-item highlight">
                    <mat-icon>sell</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.annonce.pricePerUnit' | translate }}</span>
                      <span class="info-value price">{{ a.pricePerUnit | number }} FCFA</span>
                    </div>
                  </div>
                  <div class="info-item">
                    <mat-icon>location_on</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.annonce.location' | translate }}</span>
                      <span class="info-value">{{ a.location }}</span>
                    </div>
                  </div>
                  <div class="info-item">
                    <mat-icon>date_range</mat-icon>
                    <div>
                      <span class="info-label">{{ 'marketplace.annonce.availabilityPeriod' | translate }}</span>
                      <span class="info-value">{{ a.availabilityStart | date:'shortDate' }} - {{ a.availabilityEnd | date:'shortDate' }}</span>
                    </div>
                  </div>
                </div>

                <mat-divider></mat-divider>

                <!-- Description -->
                <div class="description-section">
                  <h3>{{ 'marketplace.annonce.description' | translate }}</h3>
                  <p>{{ a.description }}</p>
                </div>

                <!-- Actions -->
                <div class="action-buttons">
                  <button mat-raised-button color="primary" class="action-btn">
                    <mat-icon>shopping_cart</mat-icon>
                    {{ 'marketplace.annonce.commander' | translate }}
                  </button>
                  <button mat-raised-button color="accent" class="action-btn">
                    <mat-icon>chat</mat-icon>
                    {{ 'marketplace.annonce.contacter' | translate }}
                  </button>
                </div>
              </mat-card-content>
            </mat-card>
          </div>

          <!-- Sidebar: Eleveur Profile -->
          <div class="sidebar">
            <mat-card class="eleveur-card">
              <mat-card-header>
                <mat-icon mat-card-avatar class="eleveur-avatar">person</mat-icon>
                <mat-card-title>{{ a.eleveur.nom }} {{ a.eleveur.prenom || '' }}</mat-card-title>
                <mat-card-subtitle>{{ a.eleveur.localisation }}</mat-card-subtitle>
              </mat-card-header>
              <mat-card-content>
                <div class="eleveur-stats">
                  <div class="stat">
                    <mat-icon class="star-color">star</mat-icon>
                    <span class="stat-value">{{ a.eleveur.note | number:'1.1-1' }}</span>
                    <span class="stat-label">{{ 'marketplace.annonce.rating' | translate }}</span>
                  </div>
                  @if (a.eleveur.totalVentes != null) {
                    <div class="stat">
                      <mat-icon>storefront</mat-icon>
                      <span class="stat-value">{{ a.eleveur.totalVentes }}</span>
                      <span class="stat-label">{{ 'marketplace.annonce.totalSales' | translate }}</span>
                    </div>
                  }
                  @if (a.eleveur.ponctualite != null) {
                    <div class="stat">
                      <mat-icon>schedule</mat-icon>
                      <span class="stat-value">{{ a.eleveur.ponctualite }}%</span>
                      <span class="stat-label">{{ 'marketplace.annonce.onTime' | translate }}</span>
                    </div>
                  }
                  @if (a.eleveur.responseTime) {
                    <div class="stat">
                      <mat-icon>quickreply</mat-icon>
                      <span class="stat-value">{{ a.eleveur.responseTime }}</span>
                      <span class="stat-label">{{ 'marketplace.annonce.responseTime' | translate }}</span>
                    </div>
                  }
                </div>

                @if (a.eleveur.telephone) {
                  <button mat-stroked-button class="full-width-btn">
                    <mat-icon>phone</mat-icon>
                    {{ a.eleveur.telephone }}
                  </button>
                }
              </mat-card-content>
            </mat-card>

            <!-- Veterinary Certificate -->
            @if (a.ficheSanitaireId) {
              <mat-card class="vet-card">
                <mat-card-header>
                  <mat-icon mat-card-avatar class="vet-avatar">medical_services</mat-icon>
                  <mat-card-title>{{ 'marketplace.annonce.vetCertificate' | translate }}</mat-card-title>
                </mat-card-header>
                <mat-card-content>
                  <p class="vet-status"
                    [class.verified]="a.veterinaryStatus === 'VERIFIED'"
                    [class.pending]="a.veterinaryStatus === 'PENDING'">
                    @if (a.veterinaryStatus === 'VERIFIED') {
                      <mat-icon>verified</mat-icon> {{ 'marketplace.annonce.vetVerifiedFull' | translate }}
                    } @else if (a.veterinaryStatus === 'PENDING') {
                      <mat-icon>pending</mat-icon> {{ 'marketplace.annonce.vetPendingFull' | translate }}
                    }
                  </p>
                  <button mat-stroked-button class="full-width-btn">
                    <mat-icon>description</mat-icon>
                    {{ 'marketplace.annonce.viewFiche' | translate }}
                  </button>
                </mat-card-content>
              </mat-card>
            }
          </div>
        </div>

        <!-- Similar Annonces -->
        @if (similarAnnonces().length > 0) {
          <section class="similar-section">
            <h2>
              <mat-icon>recommend</mat-icon>
              {{ 'marketplace.annonce.similar' | translate }}
            </h2>
            <div class="similar-grid">
              @for (sim of similarAnnonces(); track sim.id) {
                <mat-card class="similar-card" [routerLink]="['/marketplace/annonces', sim.id]">
                  <mat-card-header>
                    <mat-icon mat-card-avatar class="race-avatar-sm">egg_alt</mat-icon>
                    <mat-card-title>{{ sim.race }}</mat-card-title>
                    <mat-card-subtitle>{{ sim.eleveur.nom }} - {{ sim.location }}</mat-card-subtitle>
                  </mat-card-header>
                  <mat-card-content>
                    <div class="similar-details">
                      <span>{{ sim.quantity }} {{ 'marketplace.annonce.units' | translate }}</span>
                      <span class="price">{{ sim.pricePerKg | number }} FCFA/kg</span>
                    </div>
                  </mat-card-content>
                </mat-card>
              }
            </div>
          </section>
        }
      }
    </div>
  `,
  styles: [`
    .annonce-detail-page {
      padding: 24px;
      max-width: 1200px;
      margin: 0 auto;
    }

    .breadcrumb {
      display: flex;
      align-items: center;
      gap: 4px;
      margin-bottom: 24px;
      font-size: 0.9rem;
    }

    .breadcrumb a {
      color: #1976d2;
      text-decoration: none;
    }

    .breadcrumb a:hover {
      text-decoration: underline;
    }

    .breadcrumb mat-icon {
      font-size: 18px;
      width: 18px;
      height: 18px;
      color: #999;
    }

    .detail-layout {
      display: grid;
      grid-template-columns: 1fr 350px;
      gap: 24px;
    }

    @media (max-width: 960px) {
      .detail-layout {
        grid-template-columns: 1fr;
      }
    }

    .loading-container {
      display: flex;
      justify-content: center;
      padding: 80px;
    }

    /* Photo Gallery */
    .photo-card {
      margin-bottom: 20px;
    }

    .main-photo {
      width: 100%;
      max-height: 400px;
      object-fit: cover;
      border-radius: 8px;
    }

    .photo-thumbnails {
      display: flex;
      gap: 8px;
      margin-top: 12px;
      overflow-x: auto;
    }

    .photo-thumbnails img {
      width: 72px;
      height: 72px;
      object-fit: cover;
      border-radius: 4px;
      cursor: pointer;
      border: 2px solid transparent;
      transition: border-color 0.2s;
    }

    .photo-thumbnails img.active {
      border-color: #1976d2;
    }

    /* Details */
    .status-badges {
      margin: 16px 0;
    }

    .badge-verified { --mdc-chip-elevated-container-color: #e8f5e9; }
    .badge-pending { --mdc-chip-elevated-container-color: #fff3e0; }
    .badge-none { --mdc-chip-elevated-container-color: #fafafa; }
    .badge-halal { --mdc-chip-elevated-container-color: #e3f2fd; }

    .info-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
      gap: 16px;
      margin: 20px 0;
    }

    .info-item {
      display: flex;
      align-items: flex-start;
      gap: 12px;
      padding: 12px;
      border-radius: 8px;
      background: #fafafa;
    }

    .info-item.highlight {
      background: #f1f8e9;
    }

    .info-item mat-icon {
      color: #666;
      margin-top: 2px;
    }

    .info-label {
      display: block;
      font-size: 0.8rem;
      color: #888;
      margin-bottom: 2px;
    }

    .info-value {
      display: block;
      font-weight: 600;
      font-size: 1rem;
    }

    .info-value.price {
      color: #2e7d32;
    }

    .description-section {
      margin: 20px 0;
    }

    .description-section h3 {
      margin-bottom: 8px;
    }

    .action-buttons {
      display: flex;
      gap: 12px;
      margin-top: 24px;
    }

    .action-btn {
      flex: 1;
      height: 48px;
    }

    /* Sidebar */
    .sidebar {
      display: flex;
      flex-direction: column;
      gap: 20px;
    }

    .eleveur-avatar {
      color: #1976d2;
      background: #e3f2fd;
      font-size: 28px;
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: 50%;
    }

    .eleveur-stats {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 12px;
      margin: 16px 0;
    }

    .stat {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 4px;
      padding: 12px;
      background: #fafafa;
      border-radius: 8px;
    }

    .stat mat-icon {
      color: #666;
    }

    .star-color {
      color: #ff9800 !important;
    }

    .stat-value {
      font-weight: 600;
      font-size: 1.1rem;
    }

    .stat-label {
      font-size: 0.75rem;
      color: #888;
      text-align: center;
    }

    .full-width-btn {
      width: 100%;
      margin-top: 8px;
    }

    /* Vet Card */
    .vet-avatar {
      color: #4caf50;
      background: #e8f5e9;
      font-size: 28px;
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: 50%;
    }

    .vet-status {
      display: flex;
      align-items: center;
      gap: 8px;
      margin: 12px 0;
    }

    .vet-status.verified { color: #4caf50; }
    .vet-status.pending { color: #ff9800; }

    /* Similar */
    .similar-section {
      margin-top: 40px;
    }

    .similar-section h2 {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 16px;
    }

    .similar-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
      gap: 16px;
    }

    .similar-card {
      cursor: pointer;
      transition: transform 0.15s ease;
    }

    .similar-card:hover {
      transform: translateY(-2px);
    }

    .race-avatar-sm {
      color: #2e7d32;
    }

    .similar-details {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-top: 8px;
    }

    .similar-details .price {
      color: #2e7d32;
      font-weight: 600;
    }
  `],
})
export class AnnonceDetailComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);
  private readonly marketplace = inject(MarketplaceService);

  readonly loading = signal(true);
  readonly annonce = signal<Annonce | null>(null);
  readonly similarAnnonces = signal<Annonce[]>([]);
  readonly selectedPhoto = signal(0);

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id')!;
    this.marketplace.getAnnonceById(id).subscribe({
      next: (annonce) => {
        this.annonce.set(annonce);
        this.loading.set(false);
        this.loadSimilar(id);
      },
      error: () => this.loading.set(false),
    });
  }

  private loadSimilar(annonceId: string): void {
    this.marketplace.getSimilarAnnonces(annonceId, 6).subscribe({
      next: (annonces) => this.similarAnnonces.set(annonces),
    });
  }
}
