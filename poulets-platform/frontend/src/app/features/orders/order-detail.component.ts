import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatDividerModule } from '@angular/material/divider';
import { MatTableModule } from '@angular/material/table';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge.component';
import { FcfaCurrencyPipe } from '../../shared/pipes/currency.pipe';
import { Commande, CommandeStatus } from '../../shared/models/commande.model';

interface TimelineStep {
  label: string;
  icon: string;
  date?: string;
  active: boolean;
  completed: boolean;
}

@Component({
  selector: 'app-order-detail',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatDividerModule,
    MatTableModule,
    TranslateModule,
    StatusBadgeComponent,
    FcfaCurrencyPipe,
    DatePipe,
  ],
  template: `
    <div class="order-detail-container" data-testid="orders-detail">
      <div class="page-header">
        <button mat-icon-button routerLink=".." data-testid="orders-action-back">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <div>
          <h1 data-testid="orders-detail-field-numero">{{ 'orders.detail.title' | translate }} {{ order()?.numero }}</h1>
          @if (order(); as o) {
            <app-status-badge [status]="o.statut"
                              [attr.data-testid]="'orders-status-' + o.statut.toLowerCase()"></app-status-badge>
          }
        </div>
        <span class="spacer"></span>
        <a mat-raised-button color="accent" [routerLink]="['tracking']"
           data-testid="orders-action-track">
          <mat-icon>local_shipping</mat-icon>
          {{ 'orders.detail.track' | translate }}
        </a>
      </div>

      @if (order(); as o) {
        <!-- Status Timeline -->
        <mat-card class="timeline-card">
          <mat-card-header>
            <mat-card-title>{{ 'orders.detail.timeline' | translate }}</mat-card-title>
          </mat-card-header>
          <mat-card-content>
            <div class="timeline">
              @for (step of timeline(); track step.label; let i = $index; let last = $last) {
                <div class="timeline-step" [class.active]="step.active" [class.completed]="step.completed">
                  <div class="timeline-dot">
                    @if (step.completed) {
                      <mat-icon>check</mat-icon>
                    } @else {
                      <mat-icon>{{ step.icon }}</mat-icon>
                    }
                  </div>
                  @if (!last) {
                    <div class="timeline-connector" [class.completed]="step.completed"></div>
                  }
                  <div class="timeline-content">
                    <span class="timeline-label">{{ step.label | translate }}</span>
                    @if (step.date) {
                      <span class="timeline-date">{{ step.date | date:'dd/MM/yyyy HH:mm' }}</span>
                    }
                  </div>
                </div>
              }
            </div>
          </mat-card-content>
        </mat-card>

        <!-- Order Info -->
        <div class="info-grid">
          <mat-card>
            <mat-card-header>
              <mat-card-title>{{ 'orders.detail.client_info' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="info-row">
                <mat-icon>person</mat-icon>
                <span>{{ o.clientNom }}</span>
              </div>
              <div class="info-row">
                <mat-icon>phone</mat-icon>
                <span>{{ o.telephone }}</span>
              </div>
              <div class="info-row">
                <mat-icon>location_on</mat-icon>
                <span>{{ o.adresseLivraison }}</span>
              </div>
            </mat-card-content>
          </mat-card>

          <mat-card>
            <mat-card-header>
              <mat-card-title>{{ 'orders.detail.order_info' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="info-row">
                <mat-icon>calendar_today</mat-icon>
                <span>{{ 'orders.detail.created' | translate }}: {{ o.createdAt | date:'dd/MM/yyyy' }}</span>
              </div>
              <div class="info-row">
                <mat-icon>payments</mat-icon>
                <span>{{ 'orders.detail.total' | translate }}: {{ o.prixTotal | fcfa }}</span>
              </div>
              @if (o.notes) {
                <div class="info-row">
                  <mat-icon>notes</mat-icon>
                  <span>{{ o.notes }}</span>
                </div>
              }
            </mat-card-content>
          </mat-card>
        </div>

        <!-- Items Table -->
        <mat-card>
          <mat-card-header>
            <mat-card-title>{{ 'orders.detail.items' | translate }}</mat-card-title>
          </mat-card-header>
          <mat-card-content>
            <table mat-table [dataSource]="o.items" class="full-width-table">
              <ng-container matColumnDef="race">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.detail.race' | translate }}</th>
                <td mat-cell *matCellDef="let item">{{ item.race }}</td>
              </ng-container>
              <ng-container matColumnDef="quantite">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.detail.qty' | translate }}</th>
                <td mat-cell *matCellDef="let item">{{ item.quantite }}</td>
              </ng-container>
              <ng-container matColumnDef="prixUnitaire">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.detail.unit_price' | translate }}</th>
                <td mat-cell *matCellDef="let item">{{ item.prixUnitaire | fcfa }}</td>
              </ng-container>
              <ng-container matColumnDef="sousTotal">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.detail.subtotal' | translate }}</th>
                <td mat-cell *matCellDef="let item" class="amount-cell">
                  {{ item.quantite * item.prixUnitaire | fcfa }}
                </td>
              </ng-container>
              <tr mat-header-row *matHeaderRowDef="itemColumns"></tr>
              <tr mat-row *matRowDef="let row; columns: itemColumns;"></tr>
            </table>

            <div class="order-total">
              <span>{{ 'orders.detail.grand_total' | translate }}</span>
              <span class="total-amount">{{ o.prixTotal | fcfa }}</span>
            </div>
          </mat-card-content>
        </mat-card>

        <!-- Actions -->
        @if (o.statut === 'EN_ATTENTE') {
          <div class="action-bar">
            <button mat-raised-button color="primary" (click)="confirmOrder()"
                    data-testid="orders-action-confirm">
              <mat-icon>check_circle</mat-icon>
              {{ 'orders.detail.confirm' | translate }}
            </button>
            <button mat-raised-button color="warn" (click)="cancelOrder()"
                    data-testid="orders-action-cancel">
              <mat-icon>cancel</mat-icon>
              {{ 'orders.detail.cancel' | translate }}
            </button>
          </div>
        }
      }
    </div>
  `,
  styles: [`
    .order-detail-container {
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

    .timeline {
      display: flex;
      justify-content: space-between;
      padding: 24px 16px;
      overflow-x: auto;
    }

    .timeline-step {
      display: flex;
      flex-direction: column;
      align-items: center;
      position: relative;
      flex: 1;
      min-width: 100px;
    }

    .timeline-dot {
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

    .timeline-step.active .timeline-dot {
      background: var(--faso-primary, #2e7d32);
      color: white;
    }

    .timeline-step.completed .timeline-dot {
      background: var(--faso-primary, #2e7d32);
      color: white;
    }

    .timeline-connector {
      position: absolute;
      top: 20px;
      left: 50%;
      width: 100%;
      height: 3px;
      background: #e0e0e0;

      &.completed { background: var(--faso-primary, #2e7d32); }
    }

    .timeline-content {
      display: flex;
      flex-direction: column;
      align-items: center;
      margin-top: 8px;

      .timeline-label { font-size: 0.8rem; font-weight: 500; text-align: center; }
      .timeline-date { font-size: 0.7rem; color: #999; margin-top: 2px; }
    }

    .info-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
      margin-bottom: 24px;
    }

    .info-row {
      display: flex;
      align-items: center;
      gap: 12px;
      padding: 8px 0;
      font-size: 0.9rem;

      mat-icon { color: #666; font-size: 20px; width: 20px; height: 20px; }
    }

    .full-width-table { width: 100%; }
    .amount-cell { font-weight: 500; }

    .order-total {
      display: flex;
      justify-content: flex-end;
      gap: 24px;
      padding: 16px 0;
      font-size: 1.1rem;

      .total-amount { font-weight: 700; color: var(--faso-primary-dark, #1b5e20); }
    }

    .action-bar {
      display: flex;
      gap: 12px;
      justify-content: flex-end;
      margin-top: 24px;
    }

    @media (max-width: 768px) {
      .info-grid { grid-template-columns: 1fr; }
    }
  `],
})
export class OrderDetailComponent implements OnInit {
  readonly order = signal<Commande | null>(null);
  readonly timeline = signal<TimelineStep[]>([]);
  readonly itemColumns = ['race', 'quantite', 'prixUnitaire', 'sousTotal'];

  constructor(private readonly route: ActivatedRoute) {}

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id');
    this.loadOrder(id!);
  }

  confirmOrder(): void {
    // TODO: API call to confirm
    console.log('Confirming order');
  }

  cancelOrder(): void {
    // TODO: API call to cancel
    console.log('Cancelling order');
  }

  private loadOrder(id: string): void {
    const order: Commande = {
      id, numero: 'CMD-2026-001', clientId: 'c1', clientNom: 'Restaurant Le Sahel',
      eleveurId: 'e1', eleveurNom: 'Ferme Ouedraogo',
      items: [
        { id: 'i1', race: 'Poulet bicyclette', quantite: 30, prixUnitaire: 3500, poidsMoyen: 2.1 },
        { id: 'i2', race: 'Pintade', quantite: 20, prixUnitaire: 4000, poidsMoyen: 1.8 },
      ],
      statut: CommandeStatus.EN_PREPARATION, prixTotal: 185000,
      adresseLivraison: 'Ouagadougou, Secteur 15, Avenue de la Liberte',
      telephone: '+226 70 12 34 56',
      notes: 'Livraison avant 10h le matin',
      createdAt: '2026-04-05T08:30:00',
    };
    this.order.set(order);
    this.buildTimeline(order.statut);
  }

  private buildTimeline(statut: CommandeStatus): void {
    const steps: { key: string; label: string; icon: string; date?: string }[] = [
      { key: 'EN_ATTENTE', label: 'orders.timeline.negotiation', icon: 'handshake', date: '2026-04-05T08:30:00' },
      { key: 'CONFIRMEE', label: 'orders.timeline.confirmed', icon: 'check_circle', date: '2026-04-05T14:00:00' },
      { key: 'EN_PREPARATION', label: 'orders.timeline.preparing', icon: 'inventory', date: '2026-04-06T07:00:00' },
      { key: 'PRET', label: 'orders.timeline.ready', icon: 'done_all' },
      { key: 'EN_LIVRAISON', label: 'orders.timeline.delivery', icon: 'local_shipping' },
      { key: 'LIVREE', label: 'orders.timeline.delivered', icon: 'where_to_vote' },
    ];

    const statusOrder = ['EN_ATTENTE', 'CONFIRMEE', 'EN_PREPARATION', 'PRET', 'EN_LIVRAISON', 'LIVREE'];
    const currentIdx = statusOrder.indexOf(statut);

    this.timeline.set(steps.map((step, i) => ({
      label: step.label,
      icon: step.icon,
      date: step.date,
      active: i === currentIdx,
      completed: i < currentIdx,
    })));
  }
}
