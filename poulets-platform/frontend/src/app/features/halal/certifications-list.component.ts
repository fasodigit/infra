import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatTableModule } from '@angular/material/table';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';
import { CertificationHalal } from '@shared/models/halal.model';

@Component({
  selector: 'app-certifications-list',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatTableModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
  ],
  template: `
    <div class="certifications-container">
      <div class="page-header">
        <h1>{{ 'halal.list.title' | translate }}</h1>
        <a mat-raised-button color="primary" routerLink="request">
          <mat-icon>add</mat-icon>
          {{ 'halal.list.request' | translate }}
        </a>
      </div>

      <!-- Status Summary -->
      <div class="summary-grid">
        <mat-card class="summary-card valid">
          <mat-card-content>
            <mat-icon>verified</mat-icon>
            <div>
              <span class="summary-value">{{ countByStatus('VALIDE') }}</span>
              <span class="summary-label">{{ 'halal.status.valid' | translate }}</span>
            </div>
          </mat-card-content>
        </mat-card>
        <mat-card class="summary-card pending">
          <mat-card-content>
            <mat-icon>pending</mat-icon>
            <div>
              <span class="summary-value">{{ countByStatus('EN_ATTENTE') }}</span>
              <span class="summary-label">{{ 'halal.status.pending' | translate }}</span>
            </div>
          </mat-card-content>
        </mat-card>
        <mat-card class="summary-card expired">
          <mat-card-content>
            <mat-icon>warning</mat-icon>
            <div>
              <span class="summary-value">{{ countByStatus('EXPIRE') }}</span>
              <span class="summary-label">{{ 'halal.status.expired' | translate }}</span>
            </div>
          </mat-card-content>
        </mat-card>
      </div>

      <!-- Certifications Table -->
      <mat-card>
        <mat-card-content>
          @if (certifications().length > 0) {
            <table mat-table [dataSource]="certifications()" class="full-width-table">
              <ng-container matColumnDef="numero">
                <th mat-header-cell *matHeaderCellDef>{{ 'halal.table.number' | translate }}</th>
                <td mat-cell *matCellDef="let c">
                  <a [routerLink]="[c.id]" class="cert-link">{{ c.numero }}</a>
                </td>
              </ng-container>

              <ng-container matColumnDef="abattoir">
                <th mat-header-cell *matHeaderCellDef>{{ 'halal.table.abattoir' | translate }}</th>
                <td mat-cell *matCellDef="let c">{{ c.abattoir.nom }}</td>
              </ng-container>

              <ng-container matColumnDef="dateCertification">
                <th mat-header-cell *matHeaderCellDef>{{ 'halal.table.cert_date' | translate }}</th>
                <td mat-cell *matCellDef="let c">{{ c.dateCertification | date:'dd/MM/yyyy' }}</td>
              </ng-container>

              <ng-container matColumnDef="dateExpiration">
                <th mat-header-cell *matHeaderCellDef>{{ 'halal.table.exp_date' | translate }}</th>
                <td mat-cell *matCellDef="let c">
                  <span [class.expired-date]="isExpired(c.dateExpiration)">
                    {{ c.dateExpiration | date:'dd/MM/yyyy' }}
                  </span>
                </td>
              </ng-container>

              <ng-container matColumnDef="statut">
                <th mat-header-cell *matHeaderCellDef>{{ 'halal.table.status' | translate }}</th>
                <td mat-cell *matCellDef="let c">
                  <app-status-badge [status]="c.statut"></app-status-badge>
                </td>
              </ng-container>

              <ng-container matColumnDef="actions">
                <th mat-header-cell *matHeaderCellDef></th>
                <td mat-cell *matCellDef="let c">
                  <a mat-icon-button [routerLink]="[c.id]">
                    <mat-icon>visibility</mat-icon>
                  </a>
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="displayedColumns"></tr>
              <tr mat-row *matRowDef="let row; columns: displayedColumns;" class="clickable-row"></tr>
            </table>
          } @else {
            <div class="empty-state">
              <mat-icon>verified</mat-icon>
              <p>{{ 'halal.list.empty' | translate }}</p>
            </div>
          }
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .certifications-container {
      padding: 24px;
      max-width: 1100px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .summary-grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 16px;
      margin-bottom: 24px;
    }

    .summary-card {
      mat-card-content {
        display: flex;
        align-items: center;
        gap: 12px;
      }

      .summary-value { font-size: 1.5rem; font-weight: 700; }
      .summary-label { font-size: 0.85rem; color: #666; display: block; }

      &.valid { border-left: 4px solid #4caf50; mat-icon { color: #4caf50; } }
      &.pending { border-left: 4px solid #ff9800; mat-icon { color: #ff9800; } }
      &.expired { border-left: 4px solid #f44336; mat-icon { color: #f44336; } }
    }

    .full-width-table { width: 100%; }

    .cert-link {
      color: var(--faso-primary, #2e7d32);
      text-decoration: none;
      font-weight: 500;
      &:hover { text-decoration: underline; }
    }

    .expired-date { color: #f44336; font-weight: 500; }

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
export class CertificationsListComponent implements OnInit {
  readonly certifications = signal<CertificationHalal[]>([]);
  readonly displayedColumns = ['numero', 'abattoir', 'dateCertification', 'dateExpiration', 'statut', 'actions'];

  ngOnInit(): void {
    this.loadCertifications();
  }

  countByStatus(status: string): number {
    return this.certifications().filter(c => c.statut === status).length;
  }

  isExpired(dateStr: string): boolean {
    return new Date(dateStr) < new Date();
  }

  private loadCertifications(): void {
    this.certifications.set([
      {
        id: 'cert-1', numero: 'HALAL-2026-001', lotId: 'lot-1',
        abattoir: { id: 'ab1', nom: 'Abattoir Moderne de Ouagadougou', adresse: 'Zone Industrielle', certifie: true, capaciteJournaliere: 500 },
        dateCertification: '2026-03-15', dateExpiration: '2026-09-15',
        statut: 'VALIDE', inspecteur: 'Imam Ouedraogo', createdAt: '2026-03-15',
      },
      {
        id: 'cert-2', numero: 'HALAL-2026-002', lotId: 'lot-2',
        abattoir: { id: 'ab2', nom: 'Abattoir de Bobo-Dioulasso', adresse: 'Route de Sikasso', certifie: true },
        dateCertification: '2026-04-01', dateExpiration: '2026-10-01',
        statut: 'EN_ATTENTE', createdAt: '2026-04-01',
      },
      {
        id: 'cert-3', numero: 'HALAL-2025-015', lotId: 'lot-3',
        abattoir: { id: 'ab1', nom: 'Abattoir Moderne de Ouagadougou', adresse: 'Zone Industrielle', certifie: true },
        dateCertification: '2025-09-01', dateExpiration: '2026-03-01',
        statut: 'EXPIRE', inspecteur: 'Imam Traore', createdAt: '2025-09-01',
      },
    ]);
  }
}
