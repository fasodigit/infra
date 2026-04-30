import { Component, OnInit, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatTabsModule } from '@angular/material/tabs';
import { MatChipsModule } from '@angular/material/chips';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatTooltipModule } from '@angular/material/tooltip';
import { TranslateModule } from '@ngx-translate/core';

import { ContractsService } from '../services/contracts.service';
import {
  Contract,
  ContractFilter,
} from '../../../shared/models/contract.models';

type TabKey = 'active' | 'pending' | 'expired';

@Component({
  selector: 'app-contracts-list',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatTabsModule,
    MatChipsModule,
    MatProgressSpinnerModule,
    MatTooltipModule,
    TranslateModule,
  ],
  template: `
    <div class="contracts-page" data-testid="contracts-page">
      <div class="page-header">
        <h1>
          <mat-icon>description</mat-icon>
          {{ 'contracts.title' | translate }}
        </h1>
        <a mat-raised-button color="primary" routerLink="/contracts/new"
           data-testid="contracts-action-create">
          <mat-icon>add</mat-icon>
          {{ 'contracts.create' | translate }}
        </a>
      </div>

      <mat-tab-group (selectedTabChange)="onTabChange($event.index)" animationDuration="200ms"
                     data-testid="contracts-filter-tabs">
        <!-- Active Tab -->
        <mat-tab>
          <ng-template mat-tab-label>
            <mat-icon>check_circle</mat-icon>
            <span class="tab-label">{{ 'contracts.tabs.active' | translate }}</span>
            @if (activeCount() > 0) {
              <span class="tab-badge active">{{ activeCount() }}</span>
            }
          </ng-template>

          <div class="tab-content">
            @if (loadingActive()) {
              <div class="loading-container">
                <mat-spinner diameter="40"></mat-spinner>
              </div>
            } @else if (activeContracts().length === 0) {
              <div class="empty-state" data-testid="contracts-empty-active">
                <mat-icon>inbox</mat-icon>
                <p>{{ 'contracts.noActiveContracts' | translate }}</p>
              </div>
            } @else {
              <div class="contracts-grid" data-testid="contracts-list">
                @for (contract of activeContracts(); track contract.id) {
                  <ng-container *ngTemplateOutlet="contractCard; context: { $implicit: contract }"></ng-container>
                }
              </div>
            }
          </div>
        </mat-tab>

        <!-- Pending Tab -->
        <mat-tab>
          <ng-template mat-tab-label>
            <mat-icon>pending</mat-icon>
            <span class="tab-label">{{ 'contracts.tabs.pending' | translate }}</span>
            @if (pendingCount() > 0) {
              <span class="tab-badge pending">{{ pendingCount() }}</span>
            }
          </ng-template>

          <div class="tab-content">
            @if (loadingPending()) {
              <div class="loading-container">
                <mat-spinner diameter="40"></mat-spinner>
              </div>
            } @else if (pendingContracts().length === 0) {
              <div class="empty-state">
                <mat-icon>inbox</mat-icon>
                <p>{{ 'contracts.noPendingContracts' | translate }}</p>
              </div>
            } @else {
              <div class="contracts-grid">
                @for (contract of pendingContracts(); track contract.id) {
                  <ng-container *ngTemplateOutlet="contractCard; context: { $implicit: contract }"></ng-container>
                }
              </div>
            }
          </div>
        </mat-tab>

        <!-- Expired Tab -->
        <mat-tab>
          <ng-template mat-tab-label>
            <mat-icon>history</mat-icon>
            <span class="tab-label">{{ 'contracts.tabs.expired' | translate }}</span>
          </ng-template>

          <div class="tab-content">
            @if (loadingExpired()) {
              <div class="loading-container">
                <mat-spinner diameter="40"></mat-spinner>
              </div>
            } @else if (expiredContracts().length === 0) {
              <div class="empty-state">
                <mat-icon>inbox</mat-icon>
                <p>{{ 'contracts.noExpiredContracts' | translate }}</p>
              </div>
            } @else {
              <div class="contracts-grid">
                @for (contract of expiredContracts(); track contract.id) {
                  <ng-container *ngTemplateOutlet="contractCard; context: { $implicit: contract }"></ng-container>
                }
              </div>
            }
          </div>
        </mat-tab>
      </mat-tab-group>

      <!-- Contract Card Template -->
      <ng-template #contractCard let-contract>
        <mat-card class="contract-card" [routerLink]="['/contracts', contract.id]"
                  [attr.data-testid]="'contracts-list-item-' + contract.id">
          <mat-card-header>
            <mat-icon mat-card-avatar class="contract-avatar"
              [class]="'role-' + contract.partnerRole">
              {{ contract.partnerRole === 'eleveur' ? 'agriculture' : 'store' }}
            </mat-icon>
            <mat-card-title>{{ contract.partnerName }}</mat-card-title>
            <mat-card-subtitle>
              {{ contract.race }} - {{ 'contracts.frequency.' + contract.frequency | translate }}
            </mat-card-subtitle>
          </mat-card-header>

          <mat-card-content>
            <div class="contract-details">
              <div class="detail-row">
                <span class="label">{{ 'contracts.quantityPerDelivery' | translate }}</span>
                <span class="value">{{ contract.quantityPerDelivery }}</span>
              </div>
              <div class="detail-row">
                <span class="label">{{ 'contracts.pricePerKg' | translate }}</span>
                <span class="value price">{{ contract.pricePerKg | number }} FCFA/kg</span>
              </div>
              <div class="detail-row">
                <span class="label">{{ 'contracts.period' | translate }}</span>
                <span class="value">
                  {{ contract.startDate | date:'shortDate' }} - {{ contract.endDate | date:'shortDate' }}
                </span>
              </div>
              <div class="detail-row">
                <span class="label">{{ 'contracts.duration' | translate }}</span>
                <span class="value">{{ 'contracts.duration.' + contract.duration | translate }}</span>
              </div>
            </div>

            <div class="contract-badges">
              <mat-chip-set>
                <mat-chip [class]="'status-' + contract.status.toLowerCase()"
                          [attr.data-testid]="'contracts-status-' + contract.status.toLowerCase()">
                  {{ 'contracts.status.' + contract.status | translate }}
                </mat-chip>
                @if (contract.halalRequired) {
                  <mat-chip class="badge-halal"
                    matTooltip="{{ 'contracts.halalRequired' | translate }}">
                    <mat-icon>check_circle</mat-icon>
                    {{ 'contracts.halal' | translate }}
                  </mat-chip>
                }
                @if (contract.veterinaryCertificationRequired) {
                  <mat-chip class="badge-vet"
                    matTooltip="{{ 'contracts.vetRequired' | translate }}">
                    <mat-icon>verified</mat-icon>
                    {{ 'contracts.vet' | translate }}
                  </mat-chip>
                }
              </mat-chip-set>
            </div>

            <div class="signature-status">
              <div class="sig-item" [class.signed]="contract.signedByInitiator">
                <mat-icon>{{ contract.signedByInitiator ? 'check' : 'close' }}</mat-icon>
                {{ 'contracts.signedByYou' | translate }}
              </div>
              <div class="sig-item" [class.signed]="contract.signedByPartner">
                <mat-icon>{{ contract.signedByPartner ? 'check' : 'close' }}</mat-icon>
                {{ 'contracts.signedByPartner' | translate }}
              </div>
            </div>
          </mat-card-content>
        </mat-card>
      </ng-template>
    </div>
  `,
  styles: [`
    .contracts-page {
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

    .tab-label {
      margin: 0 8px;
    }

    .tab-badge {
      font-size: 0.75rem;
      padding: 2px 8px;
      border-radius: 12px;
      color: white;
      font-weight: 600;
    }

    .tab-badge.active { background: #4caf50; }
    .tab-badge.pending { background: #ff9800; }

    .tab-content {
      padding: 24px 0;
    }

    .contracts-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
      gap: 20px;
    }

    .contract-card {
      cursor: pointer;
      transition: transform 0.15s ease, box-shadow 0.15s ease;
    }

    .contract-card:hover {
      transform: translateY(-3px);
      box-shadow: 0 6px 16px rgba(0, 0, 0, 0.15);
    }

    .contract-avatar {
      font-size: 28px;
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: 50%;
    }

    .contract-avatar.role-eleveur {
      color: #2e7d32;
      background: #e8f5e9;
    }

    .contract-avatar.role-client {
      color: #1565c0;
      background: #e3f2fd;
    }

    .contract-details {
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

    .contract-badges {
      margin: 12px 0;
    }

    .status-actif { --mdc-chip-elevated-container-color: #e8f5e9; }
    .status-en_attente { --mdc-chip-elevated-container-color: #fff3e0; }
    .status-brouillon { --mdc-chip-elevated-container-color: #f5f5f5; }
    .status-suspendu { --mdc-chip-elevated-container-color: #fff3e0; }
    .status-expire { --mdc-chip-elevated-container-color: #fafafa; }
    .status-resilie { --mdc-chip-elevated-container-color: #ffebee; }
    .badge-halal { --mdc-chip-elevated-container-color: #e3f2fd; }
    .badge-vet { --mdc-chip-elevated-container-color: #e8f5e9; }

    .signature-status {
      display: flex;
      gap: 16px;
      margin-top: 8px;
    }

    .sig-item {
      display: flex;
      align-items: center;
      gap: 4px;
      font-size: 0.8rem;
      color: #999;
    }

    .sig-item mat-icon {
      font-size: 16px;
      width: 16px;
      height: 16px;
    }

    .sig-item.signed {
      color: #4caf50;
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
  `],
})
export class ContractsListComponent implements OnInit {
  private readonly contractsService = inject(ContractsService);

  readonly activeContracts = signal<Contract[]>([]);
  readonly pendingContracts = signal<Contract[]>([]);
  readonly expiredContracts = signal<Contract[]>([]);
  readonly activeCount = signal(0);
  readonly pendingCount = signal(0);
  readonly loadingActive = signal(true);
  readonly loadingPending = signal(true);
  readonly loadingExpired = signal(true);

  ngOnInit(): void {
    this.loadActive();
    this.loadPending();
  }

  onTabChange(index: number): void {
    if (index === 2 && this.expiredContracts().length === 0 && this.loadingExpired()) {
      this.loadExpired();
    }
  }

  private loadActive(): void {
    this.loadingActive.set(true);
    const filter: ContractFilter = { status: 'ACTIF' };
    this.contractsService.getContracts(filter).subscribe({
      next: (page) => {
        this.activeContracts.set(page.content);
        this.activeCount.set(page.totalElements);
        this.loadingActive.set(false);
      },
      error: () => this.loadingActive.set(false),
    });
  }

  private loadPending(): void {
    this.loadingPending.set(true);
    const filter: ContractFilter = { status: 'EN_ATTENTE' };
    this.contractsService.getContracts(filter).subscribe({
      next: (page) => {
        this.pendingContracts.set(page.content);
        this.pendingCount.set(page.totalElements);
        this.loadingPending.set(false);
      },
      error: () => this.loadingPending.set(false),
    });
  }

  private loadExpired(): void {
    this.loadingExpired.set(true);
    const filter: ContractFilter = { status: 'EXPIRE' };
    this.contractsService.getContracts(filter).subscribe({
      next: (page) => {
        this.expiredContracts.set(page.content);
        this.loadingExpired.set(false);
      },
      error: () => this.loadingExpired.set(false),
    });
  }
}
