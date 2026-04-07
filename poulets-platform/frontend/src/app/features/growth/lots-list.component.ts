import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe, DecimalPipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatTableModule } from '@angular/material/table';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge.component';
import { Lot, Race } from '../../shared/models/poulet.model';

@Component({
  selector: 'app-lots-list',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatTableModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    MatProgressBarModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
    DecimalPipe,
  ],
  template: `
    <div class="lots-container">
      <div class="page-header">
        <h1>{{ 'growth.lots.title' | translate }}</h1>
      </div>

      <!-- Summary Cards -->
      <div class="summary-grid">
        <mat-card class="summary-card">
          <mat-card-content>
            <mat-icon>inventory_2</mat-icon>
            <div class="summary-info">
              <span class="summary-value">{{ activeLots().length }}</span>
              <span class="summary-label">{{ 'growth.lots.active_lots' | translate }}</span>
            </div>
          </mat-card-content>
        </mat-card>
        <mat-card class="summary-card">
          <mat-card-content>
            <mat-icon>egg_alt</mat-icon>
            <div class="summary-info">
              <span class="summary-value">{{ totalBirds() }}</span>
              <span class="summary-label">{{ 'growth.lots.total_birds' | translate }}</span>
            </div>
          </mat-card-content>
        </mat-card>
        <mat-card class="summary-card">
          <mat-card-content>
            <mat-icon>monitor_weight</mat-icon>
            <div class="summary-info">
              <span class="summary-value">{{ avgWeight() | number:'1.2-2' }} kg</span>
              <span class="summary-label">{{ 'growth.lots.avg_weight' | translate }}</span>
            </div>
          </mat-card-content>
        </mat-card>
      </div>

      <!-- Lots Table -->
      <mat-card>
        <mat-card-content>
          @if (lots().length > 0) {
            <table mat-table [dataSource]="lots()" class="full-width-table">
              <ng-container matColumnDef="nom">
                <th mat-header-cell *matHeaderCellDef>{{ 'growth.lots.name' | translate }}</th>
                <td mat-cell *matCellDef="let lot">
                  <a [routerLink]="[lot.id]" class="lot-link">{{ lot.nom }}</a>
                </td>
              </ng-container>

              <ng-container matColumnDef="race">
                <th mat-header-cell *matHeaderCellDef>{{ 'growth.lots.race' | translate }}</th>
                <td mat-cell *matCellDef="let lot">{{ lot.race }}</td>
              </ng-container>

              <ng-container matColumnDef="effectif">
                <th mat-header-cell *matHeaderCellDef>{{ 'growth.lots.count' | translate }}</th>
                <td mat-cell *matCellDef="let lot">
                  {{ lot.effectifActuel }} / {{ lot.effectifInitial }}
                </td>
              </ng-container>

              <ng-container matColumnDef="poidsMoyen">
                <th mat-header-cell *matHeaderCellDef>{{ 'growth.lots.avg_weight' | translate }}</th>
                <td mat-cell *matCellDef="let lot">{{ lot.poidsMoyen | number:'1.2-2' }} kg</td>
              </ng-container>

              <ng-container matColumnDef="progression">
                <th mat-header-cell *matHeaderCellDef>{{ 'growth.lots.progress' | translate }}</th>
                <td mat-cell *matCellDef="let lot" style="width: 150px">
                  <mat-progress-bar mode="determinate"
                    [value]="getWeightProgress(lot)">
                  </mat-progress-bar>
                  <span class="progress-text">{{ getWeightProgress(lot) | number:'1.0-0' }}%</span>
                </td>
              </ng-container>

              <ng-container matColumnDef="dateArrivee">
                <th mat-header-cell *matHeaderCellDef>{{ 'growth.lots.arrival_date' | translate }}</th>
                <td mat-cell *matCellDef="let lot">{{ lot.dateArrivee | date:'dd/MM/yyyy' }}</td>
              </ng-container>

              <ng-container matColumnDef="statut">
                <th mat-header-cell *matHeaderCellDef>{{ 'growth.lots.status' | translate }}</th>
                <td mat-cell *matCellDef="let lot">
                  <app-status-badge [status]="lot.statut"></app-status-badge>
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="displayedColumns"></tr>
              <tr mat-row *matRowDef="let row; columns: displayedColumns;" class="clickable-row"></tr>
            </table>
          } @else {
            <div class="empty-state">
              <mat-icon>inventory_2</mat-icon>
              <p>{{ 'growth.lots.empty' | translate }}</p>
            </div>
          }
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .lots-container {
      padding: 24px;
      max-width: 1200px;
      margin: 0 auto;
    }

    .page-header {
      margin-bottom: 24px;
      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .summary-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .summary-card mat-card-content {
      display: flex;
      align-items: center;
      gap: 16px;

      mat-icon {
        font-size: 36px;
        width: 36px;
        height: 36px;
        color: var(--faso-primary, #2e7d32);
      }
    }

    .summary-info {
      display: flex;
      flex-direction: column;

      .summary-value { font-size: 1.5rem; font-weight: 700; }
      .summary-label { font-size: 0.8rem; color: #666; }
    }

    .full-width-table { width: 100%; }

    .lot-link {
      color: var(--faso-primary, #2e7d32);
      text-decoration: none;
      font-weight: 500;
      &:hover { text-decoration: underline; }
    }

    .progress-text {
      font-size: 0.75rem;
      color: #666;
      margin-left: 8px;
    }

    .clickable-row:hover { background: rgba(0, 0, 0, 0.04); }

    .empty-state {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 48px 24px;
      color: #999;

      mat-icon { font-size: 48px; width: 48px; height: 48px; margin-bottom: 16px; }
    }
  `],
})
export class LotsListComponent implements OnInit {
  readonly lots = signal<Lot[]>([]);
  readonly displayedColumns = ['nom', 'race', 'effectif', 'poidsMoyen', 'progression', 'dateArrivee', 'statut'];

  readonly activeLots = signal<Lot[]>([]);
  readonly totalBirds = signal(0);
  readonly avgWeight = signal(0);

  ngOnInit(): void {
    this.loadLots();
  }

  getWeightProgress(lot: Lot): number {
    const targetWeight = 2.5;
    return Math.min(100, (lot.poidsMoyen / targetWeight) * 100);
  }

  private loadLots(): void {
    const data: Lot[] = [
      {
        id: 'lot-1', nom: 'Lot A - Brahma', race: Race.BRAHMA,
        effectifInitial: 200, effectifActuel: 195, dateArrivee: '2026-02-15',
        ageArrivee: 1, poidsArrivee: 0.12, poidsMoyen: 1.85,
        tauxMortalite: 2.5, indiceConversion: 1.8,
        statut: 'EN_COURS', mesures: [], eleveurId: 'e1', createdAt: '2026-02-15',
      },
      {
        id: 'lot-2', nom: 'Lot B - Bicyclette', race: Race.BICYCLETTE,
        effectifInitial: 150, effectifActuel: 148, dateArrivee: '2026-03-01',
        ageArrivee: 1, poidsArrivee: 0.10, poidsMoyen: 1.20,
        tauxMortalite: 1.3, indiceConversion: 1.6,
        statut: 'EN_COURS', mesures: [], eleveurId: 'e1', createdAt: '2026-03-01',
      },
      {
        id: 'lot-3', nom: 'Lot C - Pintade', race: Race.PINTADE,
        effectifInitial: 100, effectifActuel: 0, dateArrivee: '2025-12-01',
        ageArrivee: 1, poidsArrivee: 0.08, poidsMoyen: 2.30,
        tauxMortalite: 3.0, indiceConversion: 2.0,
        statut: 'VENDU', mesures: [], eleveurId: 'e1', createdAt: '2025-12-01',
      },
    ];
    this.lots.set(data);
    const active = data.filter(l => l.statut === 'EN_COURS');
    this.activeLots.set(active);
    this.totalBirds.set(active.reduce((s, l) => s + l.effectifActuel, 0));
    const avg = active.reduce((s, l) => s + l.poidsMoyen, 0) / (active.length || 1);
    this.avgWeight.set(avg);
  }
}
