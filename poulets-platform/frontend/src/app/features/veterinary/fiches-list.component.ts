import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatTableModule } from '@angular/material/table';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatMenuModule } from '@angular/material/menu';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';
import { FicheSanitaire, StatutSanitaire } from '@shared/models/veterinaire.model';

@Component({
  selector: 'app-fiches-list',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatTableModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    MatMenuModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
  ],
  template: `
    <div class="fiches-container">
      <div class="page-header">
        <h1>{{ 'veterinary.list.title' | translate }}</h1>
        <div class="header-actions">
          <button mat-raised-button color="primary" [matMenuTriggerFor]="addMenu">
            <mat-icon>add</mat-icon>
            {{ 'veterinary.list.add_action' | translate }}
          </button>
          <mat-menu #addMenu="matMenu">
            <a mat-menu-item routerLink="vaccination/new">
              <mat-icon>vaccines</mat-icon>
              {{ 'veterinary.list.add_vaccination' | translate }}
            </a>
            <a mat-menu-item routerLink="treatment/new">
              <mat-icon>medication</mat-icon>
              {{ 'veterinary.list.add_treatment' | translate }}
            </a>
          </mat-menu>
        </div>
      </div>

      <!-- Status Summary -->
      <div class="status-grid">
        <mat-card class="status-card sain" (click)="filterByStatus('SAIN')">
          <mat-card-content>
            <mat-icon>check_circle</mat-icon>
            <span class="status-count">{{ countByStatus('SAIN') }}</span>
            <span class="status-label">{{ 'veterinary.status.healthy' | translate }}</span>
          </mat-card-content>
        </mat-card>
        <mat-card class="status-card traitement" (click)="filterByStatus('EN_TRAITEMENT')">
          <mat-card-content>
            <mat-icon>medication</mat-icon>
            <span class="status-count">{{ countByStatus('EN_TRAITEMENT') }}</span>
            <span class="status-label">{{ 'veterinary.status.treating' | translate }}</span>
          </mat-card-content>
        </mat-card>
        <mat-card class="status-card quarantaine" (click)="filterByStatus('QUARANTAINE')">
          <mat-card-content>
            <mat-icon>warning</mat-icon>
            <span class="status-count">{{ countByStatus('QUARANTAINE') }}</span>
            <span class="status-label">{{ 'veterinary.status.quarantine' | translate }}</span>
          </mat-card-content>
        </mat-card>
      </div>

      <!-- Fiches Table -->
      <mat-card>
        <mat-card-content>
          @if (filteredFiches().length > 0) {
            <table mat-table [dataSource]="filteredFiches()" class="full-width-table">
              <ng-container matColumnDef="lotNom">
                <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.table.lot' | translate }}</th>
                <td mat-cell *matCellDef="let f">
                  <a [routerLink]="[f.lotId]" class="fiche-link">{{ f.lotNom }}</a>
                </td>
              </ng-container>

              <ng-container matColumnDef="statut">
                <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.table.status' | translate }}</th>
                <td mat-cell *matCellDef="let f">
                  <app-status-badge [status]="f.statut"></app-status-badge>
                </td>
              </ng-container>

              <ng-container matColumnDef="vaccinations">
                <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.table.vaccinations' | translate }}</th>
                <td mat-cell *matCellDef="let f">{{ f.vaccinations.length }}</td>
              </ng-container>

              <ng-container matColumnDef="traitements">
                <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.table.treatments' | translate }}</th>
                <td mat-cell *matCellDef="let f">{{ f.traitements.length }}</td>
              </ng-container>

              <ng-container matColumnDef="derniereVisite">
                <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.table.last_visit' | translate }}</th>
                <td mat-cell *matCellDef="let f">{{ f.derniereVisite | date:'dd/MM/yyyy' }}</td>
              </ng-container>

              <ng-container matColumnDef="prochaineVisite">
                <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.table.next_visit' | translate }}</th>
                <td mat-cell *matCellDef="let f">
                  @if (f.prochaineVisite) {
                    <span [class.urgent]="isUrgent(f.prochaineVisite)">
                      {{ f.prochaineVisite | date:'dd/MM/yyyy' }}
                    </span>
                  } @else {
                    -
                  }
                </td>
              </ng-container>

              <ng-container matColumnDef="actions">
                <th mat-header-cell *matHeaderCellDef></th>
                <td mat-cell *matCellDef="let f">
                  <a mat-icon-button [routerLink]="[f.lotId]">
                    <mat-icon>visibility</mat-icon>
                  </a>
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="displayedColumns"></tr>
              <tr mat-row *matRowDef="let row; columns: displayedColumns;" class="clickable-row"></tr>
            </table>
          } @else {
            <div class="empty-state">
              <mat-icon>medical_services</mat-icon>
              <p>{{ 'veterinary.list.empty' | translate }}</p>
            </div>
          }
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .fiches-container {
      padding: 24px;
      max-width: 1200px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .status-grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 16px;
      margin-bottom: 24px;
    }

    .status-card {
      cursor: pointer;
      transition: transform 0.2s;

      &:hover { transform: translateY(-2px); }

      mat-card-content {
        display: flex;
        align-items: center;
        gap: 12px;
      }

      &.sain { border-left: 4px solid #4caf50; mat-icon { color: #4caf50; } }
      &.traitement { border-left: 4px solid #ff9800; mat-icon { color: #ff9800; } }
      &.quarantaine { border-left: 4px solid #f44336; mat-icon { color: #f44336; } }

      .status-count { font-size: 1.5rem; font-weight: 700; }
      .status-label { font-size: 0.85rem; color: #666; }
    }

    .full-width-table { width: 100%; }

    .fiche-link {
      color: var(--faso-primary, #2e7d32);
      text-decoration: none;
      font-weight: 500;
      &:hover { text-decoration: underline; }
    }

    .urgent { color: #f44336; font-weight: 500; }

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
export class FichesListComponent implements OnInit {
  readonly fiches = signal<FicheSanitaire[]>([]);
  readonly activeStatusFilter = signal<StatutSanitaire | null>(null);
  readonly displayedColumns = ['lotNom', 'statut', 'vaccinations', 'traitements', 'derniereVisite', 'prochaineVisite', 'actions'];

  readonly filteredFiches = signal<FicheSanitaire[]>([]);

  ngOnInit(): void {
    this.loadFiches();
  }

  filterByStatus(status: StatutSanitaire): void {
    const current = this.activeStatusFilter();
    if (current === status) {
      this.activeStatusFilter.set(null);
      this.filteredFiches.set(this.fiches());
    } else {
      this.activeStatusFilter.set(status);
      this.filteredFiches.set(this.fiches().filter(f => f.statut === status));
    }
  }

  countByStatus(status: StatutSanitaire): number {
    return this.fiches().filter(f => f.statut === status).length;
  }

  isUrgent(dateStr: string): boolean {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = date.getTime() - now.getTime();
    return diff < 7 * 24 * 60 * 60 * 1000 && diff > 0;
  }

  private loadFiches(): void {
    const data: FicheSanitaire[] = [
      {
        id: 'fs1', lotId: 'lot-1', lotNom: 'Lot A - Brahma', statut: 'SAIN',
        vaccinations: [
          { id: 'v1', nomVaccin: 'Newcastle', dateAdministration: '2026-02-20', administrePar: 'Dr. Sawadogo', prochaineDose: '2026-05-20' },
          { id: 'v2', nomVaccin: 'Gumboro', dateAdministration: '2026-03-01', administrePar: 'Dr. Sawadogo' },
        ],
        traitements: [],
        derniereVisite: '2026-03-28', prochaineVisite: '2026-04-12',
        veterinaire: 'Dr. Sawadogo', createdAt: '2026-02-15',
      },
      {
        id: 'fs2', lotId: 'lot-2', lotNom: 'Lot B - Bicyclette', statut: 'EN_TRAITEMENT',
        vaccinations: [
          { id: 'v3', nomVaccin: 'Newcastle', dateAdministration: '2026-03-05', administrePar: 'Dr. Ouedraogo' },
        ],
        traitements: [
          { id: 't1', nomTraitement: 'Antibiotique Tylosine', diagnostic: 'Mycoplasmose', dateDebut: '2026-04-01', dateFin: '2026-04-07', duree: 7, prescritPar: 'Dr. Ouedraogo' },
        ],
        derniereVisite: '2026-04-01', prochaineVisite: '2026-04-08',
        veterinaire: 'Dr. Ouedraogo', createdAt: '2026-03-01',
      },
      {
        id: 'fs3', lotId: 'lot-3', lotNom: 'Lot C - Pintade', statut: 'SAIN',
        vaccinations: [
          { id: 'v4', nomVaccin: 'Newcastle', dateAdministration: '2025-12-10', administrePar: 'Dr. Sawadogo' },
        ],
        traitements: [],
        derniereVisite: '2026-01-15',
        veterinaire: 'Dr. Sawadogo', createdAt: '2025-12-01',
      },
    ];
    this.fiches.set(data);
    this.filteredFiches.set(data);
  }
}
