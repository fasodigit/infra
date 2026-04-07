import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';
import { Livraison, ModeLivraison } from '@shared/models/livraison.model';

interface DeliveryTimeline {
  label: string;
  icon: string;
  date?: string;
  active: boolean;
  completed: boolean;
}

@Component({
  selector: 'app-delivery-detail',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatDividerModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
  ],
  template: `
    <div class="detail-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'delivery.detail.title' | translate }}</h1>
        <span class="spacer"></span>
        @if (delivery(); as d) {
          <app-status-badge [status]="d.statut"></app-status-badge>
        }
      </div>

      @if (delivery(); as d) {
        <!-- Status Timeline -->
        <mat-card class="timeline-card">
          <mat-card-content>
            <div class="delivery-timeline">
              @for (step of timeline(); track step.label; let last = $last) {
                <div class="timeline-step" [class.active]="step.active" [class.completed]="step.completed">
                  <div class="step-dot">
                    @if (step.completed) {
                      <mat-icon>check</mat-icon>
                    } @else {
                      <mat-icon>{{ step.icon }}</mat-icon>
                    }
                  </div>
                  @if (!last) {
                    <div class="step-line" [class.completed]="step.completed"></div>
                  }
                  <div class="step-info">
                    <span class="step-label">{{ step.label | translate }}</span>
                    @if (step.date) {
                      <span class="step-date">{{ step.date | date:'dd/MM HH:mm' }}</span>
                    }
                  </div>
                </div>
              }
            </div>
          </mat-card-content>
        </mat-card>

        <div class="info-grid">
          <!-- Route Info -->
          <mat-card>
            <mat-card-header>
              <mat-card-title>{{ 'delivery.detail.route' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="route-display">
                <div class="route-point">
                  <mat-icon class="origin">trip_origin</mat-icon>
                  <div>
                    <span class="point-label">{{ 'delivery.detail.from' | translate }}</span>
                    <span class="point-value">{{ d.adresseDepart }}</span>
                  </div>
                </div>
                <div class="route-line"></div>
                <div class="route-point">
                  <mat-icon class="destination">place</mat-icon>
                  <div>
                    <span class="point-label">{{ 'delivery.detail.to' | translate }}</span>
                    <span class="point-value">{{ d.adresseArrivee }}</span>
                  </div>
                </div>
              </div>
            </mat-card-content>
          </mat-card>

          <!-- Delivery Info -->
          <mat-card>
            <mat-card-header>
              <mat-card-title>{{ 'delivery.detail.info' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="info-list">
                <div class="info-item">
                  <mat-icon>event</mat-icon>
                  <span>{{ 'delivery.detail.estimated_date' | translate }}: {{ d.dateEstimee | date:'dd/MM/yyyy' }}</span>
                </div>
                <div class="info-item">
                  <mat-icon>{{ getModeIcon(d.modeLivraison) }}</mat-icon>
                  <span>{{ 'delivery.detail.mode' | translate }}: {{ d.modeLivraison }}</span>
                </div>
                @if (d.livreur) {
                  <mat-divider></mat-divider>
                  <div class="info-item">
                    <mat-icon>person</mat-icon>
                    <span>{{ d.livreur.nom }}</span>
                  </div>
                  <div class="info-item">
                    <mat-icon>phone</mat-icon>
                    <span>{{ d.livreur.telephone }}</span>
                  </div>
                }
                @if (d.notes) {
                  <mat-divider></mat-divider>
                  <div class="info-item">
                    <mat-icon>notes</mat-icon>
                    <span>{{ d.notes }}</span>
                  </div>
                }
              </div>
            </mat-card-content>
          </mat-card>
        </div>
      }
    </div>
  `,
  styles: [`
    .detail-container {
      padding: 24px;
      max-width: 1000px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
      .spacer { flex: 1; }
    }

    .timeline-card { margin-bottom: 24px; }

    .delivery-timeline {
      display: flex;
      justify-content: space-between;
      padding: 24px 16px;
    }

    .timeline-step {
      display: flex;
      flex-direction: column;
      align-items: center;
      position: relative;
      flex: 1;
    }

    .step-dot {
      width: 40px;
      height: 40px;
      border-radius: 50%;
      display: flex;
      align-items: center;
      justify-content: center;
      background: #e0e0e0;
      color: #999;
      z-index: 1;

      mat-icon { font-size: 20px; width: 20px; height: 20px; }
    }

    .timeline-step.active .step-dot,
    .timeline-step.completed .step-dot {
      background: var(--faso-primary, #2e7d32);
      color: white;
    }

    .step-line {
      position: absolute;
      top: 20px;
      left: 50%;
      width: 100%;
      height: 3px;
      background: #e0e0e0;

      &.completed { background: var(--faso-primary, #2e7d32); }
    }

    .step-info {
      display: flex;
      flex-direction: column;
      align-items: center;
      margin-top: 8px;

      .step-label { font-size: 0.8rem; font-weight: 500; text-align: center; }
      .step-date { font-size: 0.7rem; color: #999; }
    }

    .info-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 24px;
    }

    .route-display {
      display: flex;
      flex-direction: column;
      gap: 4px;
      padding: 8px 0;
    }

    .route-point {
      display: flex;
      align-items: center;
      gap: 12px;

      .origin { color: var(--faso-primary, #2e7d32); }
      .destination { color: #f44336; }

      div { display: flex; flex-direction: column; }
      .point-label { font-size: 0.75rem; color: #999; }
      .point-value { font-weight: 500; }
    }

    .route-line {
      width: 2px;
      height: 24px;
      background: #e0e0e0;
      margin-left: 11px;
    }

    .info-list {
      display: flex;
      flex-direction: column;
      gap: 12px;
    }

    .info-item {
      display: flex;
      align-items: center;
      gap: 12px;
      font-size: 0.9rem;

      mat-icon { color: #666; font-size: 20px; width: 20px; height: 20px; }
    }

    @media (max-width: 768px) {
      .info-grid { grid-template-columns: 1fr; }
    }
  `],
})
export class DeliveryDetailComponent implements OnInit {
  readonly delivery = signal<Livraison | null>(null);
  readonly timeline = signal<DeliveryTimeline[]>([]);

  constructor(private readonly route: ActivatedRoute) {}

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id');
    this.loadDelivery(id!);
  }

  getModeIcon(mode: ModeLivraison): string {
    switch (mode) {
      case ModeLivraison.MOTO: return 'two_wheeler';
      case ModeLivraison.VOITURE: return 'directions_car';
      case ModeLivraison.CAMION: return 'local_shipping';
      case ModeLivraison.RETRAIT: return 'store';
      default: return 'local_shipping';
    }
  }

  private loadDelivery(id: string): void {
    const d: Livraison = {
      id, commandeId: 'cmd-1', modeLivraison: ModeLivraison.MOTO,
      adresseDepart: 'Ferme Ouedraogo, Koudougou',
      adresseArrivee: 'Restaurant Le Sahel, Ouagadougou Secteur 15',
      dateEstimee: '2026-04-10', statut: 'EN_COURS',
      livreur: { id: 'l1', nom: 'Ibrahim Kabore', telephone: '+226 70 11 22 33', modeLivraison: ModeLivraison.MOTO, note: 4.5 },
      notes: '50 poulets bicyclette - manipuler avec soin',
      createdAt: '2026-04-05',
    };
    this.delivery.set(d);
    this.buildTimeline(d.statut);
  }

  private buildTimeline(statut: string): void {
    const steps = [
      { key: 'PLANIFIEE', label: 'delivery.timeline.planned', icon: 'event', date: '2026-04-05T10:00:00' },
      { key: 'EN_COURS', label: 'delivery.timeline.in_progress', icon: 'local_shipping', date: '2026-04-10T07:00:00' },
      { key: 'LIVREE', label: 'delivery.timeline.delivered', icon: 'where_to_vote' },
    ];
    const order = ['PLANIFIEE', 'EN_COURS', 'LIVREE'];
    const idx = order.indexOf(statut);

    this.timeline.set(steps.map((s, i) => ({
      label: s.label,
      icon: s.icon,
      date: s.date,
      active: i === idx,
      completed: i < idx,
    })));
  }
}
