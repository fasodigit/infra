import { Component, OnInit, inject, signal, computed, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatDividerModule } from '@angular/material/divider';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatTabsModule } from '@angular/material/tabs';
import { MatTableModule } from '@angular/material/table';
import { MatSnackBar, MatSnackBarModule } from '@angular/material/snack-bar';
import { MatDialogModule, MatDialog } from '@angular/material/dialog';
import { TranslateModule } from '@ngx-translate/core';

import { ContractsService } from '../services/contracts.service';
import {
  Contract,
  ContractPerformance,
  ContractDelivery,
} from '../../../shared/models/contract.models';

@Component({
  selector: 'app-contract-detail',
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
    MatProgressBarModule,
    MatTooltipModule,
    MatTabsModule,
    MatTableModule,
    MatSnackBarModule,
    MatDialogModule,
    TranslateModule,
  ],
  template: `
    <div class="contract-detail-page">
      @if (loading()) {
        <div class="loading-container">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else if (contract(); as c) {
        <!-- Breadcrumb -->
        <div class="breadcrumb">
          <a routerLink="/contracts">{{ 'contracts.title' | translate }}</a>
          <mat-icon>chevron_right</mat-icon>
          <span>{{ c.partnerName }} - {{ c.race }}</span>
        </div>

        <!-- Header Card -->
        <mat-card class="header-card" [class]="'status-border-' + c.status.toLowerCase()">
          <mat-card-content>
            <div class="header-content">
              <div class="header-info">
                <div class="header-top">
                  <mat-icon class="header-icon">description</mat-icon>
                  <div>
                    <h2>{{ c.partnerName }}</h2>
                    <p class="header-subtitle">{{ c.race }} - {{ 'contracts.frequency.' + c.frequency | translate }}</p>
                  </div>
                </div>
                <mat-chip-set>
                  <mat-chip [class]="'status-' + c.status.toLowerCase()">
                    {{ 'contracts.status.' + c.status | translate }}
                  </mat-chip>
                  @if (c.halalRequired) {
                    <mat-chip class="badge-halal">
                      <mat-icon>check_circle</mat-icon>
                      {{ 'contracts.halal' | translate }}
                    </mat-chip>
                  }
                  @if (c.veterinaryCertificationRequired) {
                    <mat-chip class="badge-vet">
                      <mat-icon>verified</mat-icon>
                      {{ 'contracts.vet' | translate }}
                    </mat-chip>
                  }
                </mat-chip-set>
              </div>

              <!-- Next Delivery Countdown -->
              @if (performance(); as perf) {
                @if (perf.nextDeliveryDate) {
                  <div class="countdown-card">
                    <span class="countdown-value">{{ perf.daysUntilNextDelivery }}</span>
                    <span class="countdown-label">{{ 'contracts.detail.daysToNext' | translate }}</span>
                    <span class="countdown-date">{{ perf.nextDeliveryDate | date:'mediumDate' }}</span>
                  </div>
                }
              }
            </div>
          </mat-card-content>
        </mat-card>

        <!-- Performance & Terms Layout -->
        <div class="detail-layout">
          <!-- Left: Terms -->
          <mat-card class="terms-card">
            <mat-card-header>
              <mat-card-title>{{ 'contracts.detail.terms' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="terms-grid">
                <div class="term-item">
                  <mat-icon>egg_alt</mat-icon>
                  <div>
                    <span class="term-label">{{ 'contracts.detail.race' | translate }}</span>
                    <span class="term-value">{{ c.race }}</span>
                  </div>
                </div>
                <div class="term-item">
                  <mat-icon>inventory_2</mat-icon>
                  <div>
                    <span class="term-label">{{ 'contracts.detail.qtyPerDelivery' | translate }}</span>
                    <span class="term-value">{{ c.quantityPerDelivery }}</span>
                  </div>
                </div>
                <div class="term-item">
                  <mat-icon>monitor_weight</mat-icon>
                  <div>
                    <span class="term-label">{{ 'contracts.detail.minWeight' | translate }}</span>
                    <span class="term-value">{{ c.minimumWeight | number:'1.1-1' }} kg</span>
                  </div>
                </div>
                <div class="term-item highlight">
                  <mat-icon>payments</mat-icon>
                  <div>
                    <span class="term-label">{{ 'contracts.detail.price' | translate }}</span>
                    <span class="term-value price">{{ c.pricePerKg | number }} FCFA/kg ({{ 'contracts.priceType.' + c.priceType | translate }})</span>
                  </div>
                </div>
                <div class="term-item">
                  <mat-icon>repeat</mat-icon>
                  <div>
                    <span class="term-label">{{ 'contracts.detail.frequency' | translate }}</span>
                    <span class="term-value">{{ 'contracts.frequency.' + c.frequency | translate }}</span>
                  </div>
                </div>
                <div class="term-item">
                  <mat-icon>date_range</mat-icon>
                  <div>
                    <span class="term-label">{{ 'contracts.detail.period' | translate }}</span>
                    <span class="term-value">{{ c.startDate | date:'shortDate' }} - {{ c.endDate | date:'shortDate' }}</span>
                  </div>
                </div>
                <div class="term-item">
                  <mat-icon>timer</mat-icon>
                  <div>
                    <span class="term-label">{{ 'contracts.detail.duration' | translate }}</span>
                    <span class="term-value">{{ 'contracts.duration.' + c.duration | translate }}</span>
                  </div>
                </div>
                <div class="term-item">
                  <mat-icon>account_balance</mat-icon>
                  <div>
                    <span class="term-label">{{ 'contracts.detail.advance' | translate }}</span>
                    <span class="term-value">{{ c.advancePaymentPercent }}%</span>
                  </div>
                </div>
                <div class="term-item">
                  <mat-icon>gavel</mat-icon>
                  <div>
                    <span class="term-label">{{ 'contracts.detail.penalties' | translate }}</span>
                    <span class="term-value">
                      {{ 'contracts.detail.lateDelivery' | translate }}: {{ c.penaltyLateDelivery }}%,
                      {{ 'contracts.detail.underWeight' | translate }}: {{ c.penaltyUnderWeight }}%
                    </span>
                  </div>
                </div>
              </div>

              <!-- Signatures -->
              <mat-divider></mat-divider>
              <div class="signatures">
                <div class="sig-item" [class.signed]="c.signedByInitiator">
                  <mat-icon>{{ c.signedByInitiator ? 'check_circle' : 'radio_button_unchecked' }}</mat-icon>
                  {{ 'contracts.detail.signedByYou' | translate }}
                </div>
                <div class="sig-item" [class.signed]="c.signedByPartner">
                  <mat-icon>{{ c.signedByPartner ? 'check_circle' : 'radio_button_unchecked' }}</mat-icon>
                  {{ 'contracts.detail.signedByPartner' | translate }}
                </div>
              </div>

              @if (!c.signedByInitiator && (c.status === 'EN_ATTENTE' || c.status === 'BROUILLON')) {
                <button mat-raised-button color="primary" class="full-width-btn"
                  (click)="signContract()" [disabled]="signing()">
                  @if (signing()) {
                    <mat-spinner diameter="20"></mat-spinner>
                  } @else {
                    <mat-icon>draw</mat-icon>
                    {{ 'contracts.detail.sign' | translate }}
                  }
                </button>
              }
            </mat-card-content>
          </mat-card>

          <!-- Right: Performance -->
          <div class="performance-column">
            @if (performance(); as perf) {
              <mat-card class="perf-card">
                <mat-card-header>
                  <mat-card-title>{{ 'contracts.detail.performance' | translate }}</mat-card-title>
                </mat-card-header>
                <mat-card-content>
                  <div class="perf-metrics">
                    <div class="metric">
                      <div class="metric-header">
                        <span>{{ 'contracts.detail.onTimeDelivery' | translate }}</span>
                        <span class="metric-value">{{ perf.onTimePercent | number:'1.0-0' }}%</span>
                      </div>
                      <mat-progress-bar mode="determinate" [value]="perf.onTimePercent"
                        [color]="perf.onTimePercent >= 80 ? 'primary' : 'warn'">
                      </mat-progress-bar>
                    </div>

                    <div class="metric">
                      <div class="metric-header">
                        <span>{{ 'contracts.detail.avgWeightVsContracted' | translate }}</span>
                        <span class="metric-value">{{ perf.averageWeightVsContracted | number:'1.0-0' }}%</span>
                      </div>
                      <mat-progress-bar mode="determinate" [value]="perf.averageWeightVsContracted"
                        [color]="perf.averageWeightVsContracted >= 95 ? 'primary' : 'warn'">
                      </mat-progress-bar>
                    </div>

                    <div class="delivery-stats">
                      <div class="stat-box">
                        <span class="stat-value">{{ perf.completedDeliveries }}</span>
                        <span class="stat-label">{{ 'contracts.detail.completed' | translate }}</span>
                      </div>
                      <div class="stat-box">
                        <span class="stat-value">{{ perf.totalDeliveries }}</span>
                        <span class="stat-label">{{ 'contracts.detail.totalDeliveries' | translate }}</span>
                      </div>
                      <div class="stat-box">
                        <span class="stat-value">{{ perf.totalDeliveries - perf.completedDeliveries }}</span>
                        <span class="stat-label">{{ 'contracts.detail.remaining' | translate }}</span>
                      </div>
                    </div>
                  </div>
                </mat-card-content>
              </mat-card>
            }

            <!-- Action Buttons -->
            <mat-card class="actions-card">
              <mat-card-content>
                @if (c.status === 'ACTIF') {
                  <button mat-raised-button color="primary" class="action-btn" (click)="renewContract()">
                    <mat-icon>autorenew</mat-icon>
                    {{ 'contracts.detail.renew' | translate }}
                  </button>
                }
                @if (c.status === 'ACTIF' || c.status === 'EN_ATTENTE') {
                  <button mat-raised-button color="warn" class="action-btn" (click)="cancelContract()">
                    <mat-icon>cancel</mat-icon>
                    {{ 'contracts.detail.cancel' | translate }}
                  </button>
                }
              </mat-card-content>
            </mat-card>
          </div>
        </div>

        <!-- Delivery History -->
        @if (c.deliveries && c.deliveries.length > 0) {
          <mat-card class="deliveries-card">
            <mat-card-header>
              <mat-card-title>
                <mat-icon>local_shipping</mat-icon>
                {{ 'contracts.detail.deliveryHistory' | translate }}
              </mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="delivery-timeline">
                @for (delivery of c.deliveries; track delivery.id) {
                  <div class="timeline-item" [class]="'delivery-' + delivery.status.toLowerCase()">
                    <div class="timeline-dot">
                      @switch (delivery.status) {
                        @case ('A_TEMPS') { <mat-icon>check_circle</mat-icon> }
                        @case ('EN_RETARD') { <mat-icon>warning</mat-icon> }
                        @case ('ANNULE') { <mat-icon>cancel</mat-icon> }
                        @default { <mat-icon>schedule</mat-icon> }
                      }
                    </div>
                    <div class="timeline-content">
                      <div class="timeline-header">
                        <span class="timeline-date">{{ delivery.scheduledDate | date:'mediumDate' }}</span>
                        <mat-chip [class]="'del-status-' + delivery.status.toLowerCase()">
                          {{ 'contracts.delivery.' + delivery.status | translate }}
                        </mat-chip>
                      </div>
                      @if (delivery.actualDate) {
                        <p class="timeline-detail">
                          {{ 'contracts.detail.deliveredOn' | translate }}: {{ delivery.actualDate | date:'mediumDate' }}
                        </p>
                      }
                      @if (delivery.quantityDelivered != null) {
                        <p class="timeline-detail">
                          {{ 'contracts.detail.qtyDelivered' | translate }}: {{ delivery.quantityDelivered }}
                          @if (delivery.averageWeight != null) {
                            ({{ delivery.averageWeight | number:'1.1-1' }} kg {{ 'contracts.detail.avg' | translate }})
                          }
                        </p>
                      }
                      @if (delivery.notes) {
                        <p class="timeline-notes">{{ delivery.notes }}</p>
                      }
                    </div>
                  </div>
                }
              </div>
            </mat-card-content>
          </mat-card>
        }
      }
    </div>
  `,
  styles: [`
    .contract-detail-page {
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

    .loading-container {
      display: flex;
      justify-content: center;
      padding: 80px;
    }

    /* Header */
    .header-card {
      margin-bottom: 24px;
      border-left: 4px solid #ccc;
    }

    .status-border-actif { border-left-color: #4caf50; }
    .status-border-en_attente { border-left-color: #ff9800; }
    .status-border-brouillon { border-left-color: #9e9e9e; }
    .status-border-expire { border-left-color: #bdbdbd; }
    .status-border-resilie { border-left-color: #f44336; }
    .status-border-suspendu { border-left-color: #ff9800; }

    .header-content {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      flex-wrap: wrap;
      gap: 20px;
    }

    .header-top {
      display: flex;
      align-items: center;
      gap: 16px;
      margin-bottom: 12px;
    }

    .header-icon {
      font-size: 40px;
      width: 40px;
      height: 40px;
      color: #1976d2;
    }

    .header-info h2 {
      margin: 0;
    }

    .header-subtitle {
      color: #666;
      margin: 0;
    }

    .status-actif { --mdc-chip-elevated-container-color: #e8f5e9; }
    .status-en_attente { --mdc-chip-elevated-container-color: #fff3e0; }
    .status-brouillon { --mdc-chip-elevated-container-color: #f5f5f5; }
    .status-expire { --mdc-chip-elevated-container-color: #fafafa; }
    .status-resilie { --mdc-chip-elevated-container-color: #ffebee; }
    .badge-halal { --mdc-chip-elevated-container-color: #e3f2fd; }
    .badge-vet { --mdc-chip-elevated-container-color: #e8f5e9; }

    .countdown-card {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 16px 24px;
      background: linear-gradient(135deg, #e3f2fd, #bbdefb);
      border-radius: 12px;
      min-width: 140px;
    }

    .countdown-value {
      font-size: 2.5rem;
      font-weight: 700;
      color: #1565c0;
      line-height: 1;
    }

    .countdown-label {
      font-size: 0.8rem;
      color: #666;
      margin-top: 4px;
    }

    .countdown-date {
      font-size: 0.75rem;
      color: #999;
      margin-top: 4px;
    }

    /* Detail Layout */
    .detail-layout {
      display: grid;
      grid-template-columns: 1fr 380px;
      gap: 24px;
      margin-bottom: 24px;
    }

    @media (max-width: 960px) {
      .detail-layout {
        grid-template-columns: 1fr;
      }
    }

    /* Terms */
    .terms-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 12px;
    }

    @media (max-width: 600px) {
      .terms-grid {
        grid-template-columns: 1fr;
      }
    }

    .term-item {
      display: flex;
      align-items: flex-start;
      gap: 12px;
      padding: 10px;
      background: #fafafa;
      border-radius: 8px;
    }

    .term-item.highlight {
      background: #f1f8e9;
    }

    .term-item mat-icon {
      color: #666;
      margin-top: 2px;
    }

    .term-label {
      display: block;
      font-size: 0.75rem;
      color: #888;
    }

    .term-value {
      display: block;
      font-weight: 500;
    }

    .term-value.price {
      color: #2e7d32;
    }

    .signatures {
      display: flex;
      gap: 24px;
      margin: 16px 0;
    }

    .sig-item {
      display: flex;
      align-items: center;
      gap: 8px;
      color: #999;
    }

    .sig-item.signed {
      color: #4caf50;
    }

    .full-width-btn {
      width: 100%;
      margin-top: 12px;
    }

    /* Performance */
    .performance-column {
      display: flex;
      flex-direction: column;
      gap: 20px;
    }

    .perf-metrics {
      display: flex;
      flex-direction: column;
      gap: 16px;
    }

    .metric-header {
      display: flex;
      justify-content: space-between;
      margin-bottom: 4px;
      font-size: 0.85rem;
    }

    .metric-value {
      font-weight: 600;
    }

    .delivery-stats {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 8px;
      margin-top: 8px;
    }

    .stat-box {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 12px;
      background: #f5f5f5;
      border-radius: 8px;
    }

    .stat-box .stat-value {
      font-size: 1.5rem;
      font-weight: 700;
      color: #333;
    }

    .stat-box .stat-label {
      font-size: 0.7rem;
      color: #888;
      text-align: center;
    }

    /* Actions */
    .actions-card mat-card-content {
      display: flex;
      flex-direction: column;
      gap: 8px;
    }

    .action-btn {
      width: 100%;
    }

    /* Delivery Timeline */
    .deliveries-card {
      margin-top: 24px;
    }

    .deliveries-card mat-card-title {
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .delivery-timeline {
      display: flex;
      flex-direction: column;
      gap: 0;
      margin-top: 16px;
    }

    .timeline-item {
      display: flex;
      gap: 16px;
      padding: 12px 0;
      border-left: 3px solid #e0e0e0;
      margin-left: 20px;
      padding-left: 20px;
      position: relative;
    }

    .timeline-item.delivery-a_temps { border-left-color: #4caf50; }
    .timeline-item.delivery-en_retard { border-left-color: #f44336; }
    .timeline-item.delivery-annule { border-left-color: #9e9e9e; }
    .timeline-item.delivery-planifie { border-left-color: #2196f3; }

    .timeline-dot {
      position: absolute;
      left: -14px;
      width: 24px;
      height: 24px;
      display: flex;
      align-items: center;
      justify-content: center;
      background: white;
    }

    .timeline-dot mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    .delivery-a_temps .timeline-dot mat-icon { color: #4caf50; }
    .delivery-en_retard .timeline-dot mat-icon { color: #f44336; }
    .delivery-annule .timeline-dot mat-icon { color: #9e9e9e; }
    .delivery-planifie .timeline-dot mat-icon { color: #2196f3; }

    .timeline-content {
      flex: 1;
    }

    .timeline-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 4px;
    }

    .timeline-date {
      font-weight: 500;
    }

    .del-status-a_temps { --mdc-chip-elevated-container-color: #e8f5e9; }
    .del-status-en_retard { --mdc-chip-elevated-container-color: #ffebee; }
    .del-status-annule { --mdc-chip-elevated-container-color: #f5f5f5; }
    .del-status-planifie { --mdc-chip-elevated-container-color: #e3f2fd; }

    .timeline-detail {
      font-size: 0.85rem;
      color: #666;
      margin: 2px 0;
    }

    .timeline-notes {
      font-size: 0.8rem;
      color: #888;
      font-style: italic;
      margin: 4px 0 0;
    }
  `],
})
export class ContractDetailComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);
  private readonly contractsService = inject(ContractsService);
  private readonly snackBar = inject(MatSnackBar);

  readonly loading = signal(true);
  readonly contract = signal<Contract | null>(null);
  readonly performance = signal<ContractPerformance | null>(null);
  readonly signing = signal(false);

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id')!;
    this.contractsService.getContractById(id).subscribe({
      next: (contract) => {
        this.contract.set(contract);
        this.loading.set(false);
        this.loadPerformance(id);
      },
      error: () => this.loading.set(false),
    });
  }

  signContract(): void {
    const c = this.contract();
    if (!c) return;
    this.signing.set(true);
    this.contractsService.signContract(c.id).subscribe({
      next: (updated) => {
        this.contract.set({ ...c, ...updated });
        this.signing.set(false);
        this.snackBar.open('Contrat signe avec succes', 'OK', { duration: 3000 });
      },
      error: () => {
        this.signing.set(false);
        this.snackBar.open('Erreur lors de la signature', 'OK', { duration: 3000 });
      },
    });
  }

  renewContract(): void {
    const c = this.contract();
    if (!c) return;
    this.contractsService.renewContract(c.id, c.duration).subscribe({
      next: (updated) => {
        this.contract.set({ ...c, ...updated });
        this.snackBar.open('Contrat renouvele avec succes', 'OK', { duration: 3000 });
      },
      error: () => {
        this.snackBar.open('Erreur lors du renouvellement', 'OK', { duration: 3000 });
      },
    });
  }

  cancelContract(): void {
    const c = this.contract();
    if (!c) return;
    this.contractsService.terminateContract(c.id, 'Annulation par utilisateur').subscribe({
      next: (updated) => {
        this.contract.set({ ...c, ...updated });
        this.snackBar.open('Contrat resilie', 'OK', { duration: 3000 });
      },
      error: () => {
        this.snackBar.open('Erreur lors de la resiliation', 'OK', { duration: 3000 });
      },
    });
  }

  private loadPerformance(contractId: string): void {
    this.contractsService.getContractPerformance(contractId).subscribe({
      next: (perf) => this.performance.set(perf),
    });
  }
}
