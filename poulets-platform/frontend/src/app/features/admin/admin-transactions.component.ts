import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatTableModule } from '@angular/material/table';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatPaginatorModule } from '@angular/material/paginator';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';
import { FcfaCurrencyPipe } from '@shared/pipes/currency.pipe';

interface Transaction {
  id: string;
  numero: string;
  date: string;
  eleveur: string;
  client: string;
  race: string;
  quantite: number;
  amount: number;
  status: string;
}

@Component({
  selector: 'app-admin-transactions',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatTableModule,
    MatButtonModule,
    MatIconModule,
    MatPaginatorModule,
    TranslateModule,
    StatusBadgeComponent,
    FcfaCurrencyPipe,
    DatePipe,
  ],
  template: `
    <div class="transactions-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'admin.transactions.title' | translate }}</h1>
      </div>

      <mat-card>
        <mat-card-content>
          <table mat-table [dataSource]="transactions()" class="full-width-table">
            <ng-container matColumnDef="numero">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.number' | translate }}</th>
              <td mat-cell *matCellDef="let t">{{ t.numero }}</td>
            </ng-container>
            <ng-container matColumnDef="date">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.date' | translate }}</th>
              <td mat-cell *matCellDef="let t">{{ t.date | date:'dd/MM/yyyy' }}</td>
            </ng-container>
            <ng-container matColumnDef="eleveur">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.eleveur' | translate }}</th>
              <td mat-cell *matCellDef="let t">{{ t.eleveur }}</td>
            </ng-container>
            <ng-container matColumnDef="client">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.client' | translate }}</th>
              <td mat-cell *matCellDef="let t">{{ t.client }}</td>
            </ng-container>
            <ng-container matColumnDef="race">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.race' | translate }}</th>
              <td mat-cell *matCellDef="let t">{{ t.race }}</td>
            </ng-container>
            <ng-container matColumnDef="quantite">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.qty' | translate }}</th>
              <td mat-cell *matCellDef="let t">{{ t.quantite }}</td>
            </ng-container>
            <ng-container matColumnDef="amount">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.amount' | translate }}</th>
              <td mat-cell *matCellDef="let t" class="amount-cell">{{ t.amount | fcfa }}</td>
            </ng-container>
            <ng-container matColumnDef="status">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.transactions.status' | translate }}</th>
              <td mat-cell *matCellDef="let t">
                <app-status-badge [status]="t.status"></app-status-badge>
              </td>
            </ng-container>
            <tr mat-header-row *matHeaderRowDef="displayedColumns"></tr>
            <tr mat-row *matRowDef="let row; columns: displayedColumns;"></tr>
          </table>
          <mat-paginator [pageSizeOptions]="[10, 25, 50]" [pageSize]="10"
                         showFirstLastButtons>
          </mat-paginator>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .transactions-container {
      padding: 24px;
      max-width: 1200px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .full-width-table { width: 100%; }
    .amount-cell { font-weight: 500; }
  `],
})
export class AdminTransactionsComponent implements OnInit {
  readonly transactions = signal<Transaction[]>([]);
  readonly displayedColumns = ['numero', 'date', 'eleveur', 'client', 'race', 'quantite', 'amount', 'status'];

  ngOnInit(): void {
    this.loadTransactions();
  }

  private loadTransactions(): void {
    this.transactions.set([
      { id: 't1', numero: 'TX-2026-001', date: '2026-04-07', eleveur: 'Ferme Ouedraogo', client: 'Restaurant Le Sahel', race: 'Poulet bicyclette', quantite: 50, amount: 175000, status: 'CONFIRMEE' },
      { id: 't2', numero: 'TX-2026-002', date: '2026-04-06', eleveur: 'Ferme Kabore', client: 'Mme Traore', race: 'Pintade', quantite: 20, amount: 80000, status: 'EN_ATTENTE' },
      { id: 't3', numero: 'TX-2026-003', date: '2026-04-05', eleveur: 'Groupement Koudougou', client: 'Hotel Splendide', race: 'Poulet de chair', quantite: 100, amount: 300000, status: 'LIVREE' },
      { id: 't4', numero: 'TX-2026-004', date: '2026-04-04', eleveur: 'Ferme Ouedraogo', client: 'M. Kabore', race: 'Poulet bicyclette', quantite: 10, amount: 35000, status: 'LIVREE' },
      { id: 't5', numero: 'TX-2026-005', date: '2026-04-03', eleveur: 'Ferme Ouedraogo', client: 'Brasserie du Centre', race: 'Coq local', quantite: 30, amount: 150000, status: 'ANNULEE' },
      { id: 't6', numero: 'TX-2026-006', date: '2026-04-02', eleveur: 'Ferme Traore', client: 'Resto Chez Papa', race: 'Dinde', quantite: 15, amount: 120000, status: 'LIVREE' },
      { id: 't7', numero: 'TX-2026-007', date: '2026-04-01', eleveur: 'Ferme Sawadogo', client: 'Supermarche Marina', race: 'Poulet fermier', quantite: 40, amount: 160000, status: 'CONFIRMEE' },
    ]);
  }
}
