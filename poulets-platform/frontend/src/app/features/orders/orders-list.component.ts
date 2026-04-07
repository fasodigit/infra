import { Component, OnInit, signal, computed } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatTableModule } from '@angular/material/table';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatSelectModule } from '@angular/material/select';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge.component';
import { FcfaCurrencyPipe } from '../../shared/pipes/currency.pipe';
import { Commande, CommandeStatus } from '../../shared/models/commande.model';

type FilterStatus = 'all' | CommandeStatus;

@Component({
  selector: 'app-orders-list',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatTableModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    MatSelectModule,
    MatFormFieldModule,
    MatInputModule,
    TranslateModule,
    StatusBadgeComponent,
    FcfaCurrencyPipe,
    DatePipe,
  ],
  template: `
    <div class="orders-container">
      <div class="page-header">
        <h1>{{ 'orders.list.title' | translate }}</h1>
        <a mat-raised-button color="primary" routerLink="new">
          <mat-icon>add</mat-icon>
          {{ 'orders.list.create' | translate }}
        </a>
      </div>

      <!-- Filters -->
      <div class="filter-bar">
        <mat-chip-listbox (change)="onFilterChange($event.value)" [value]="activeFilter()">
          <mat-chip-option value="all">
            {{ 'orders.filter.all' | translate }} ({{ orders().length }})
          </mat-chip-option>
          @for (status of statusFilters; track status.value) {
            <mat-chip-option [value]="status.value">
              {{ status.label | translate }} ({{ countByStatus(status.value) }})
            </mat-chip-option>
          }
        </mat-chip-listbox>
      </div>

      <!-- Orders Table -->
      <mat-card>
        <mat-card-content>
          @if (filteredOrders().length > 0) {
            <table mat-table [dataSource]="filteredOrders()" class="full-width-table">
              <ng-container matColumnDef="numero">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.table.number' | translate }}</th>
                <td mat-cell *matCellDef="let order">
                  <a [routerLink]="[order.id]" class="order-link">{{ order.numero }}</a>
                </td>
              </ng-container>

              <ng-container matColumnDef="partner">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.table.partner' | translate }}</th>
                <td mat-cell *matCellDef="let order">{{ order.clientNom || order.eleveurNom }}</td>
              </ng-container>

              <ng-container matColumnDef="quantite">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.table.quantity' | translate }}</th>
                <td mat-cell *matCellDef="let order">{{ getTotalQuantity(order) }}</td>
              </ng-container>

              <ng-container matColumnDef="date">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.table.date' | translate }}</th>
                <td mat-cell *matCellDef="let order">{{ order.createdAt | date:'dd/MM/yyyy' }}</td>
              </ng-container>

              <ng-container matColumnDef="montant">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.table.amount' | translate }}</th>
                <td mat-cell *matCellDef="let order" class="amount-cell">
                  {{ order.prixTotal | fcfa }}
                </td>
              </ng-container>

              <ng-container matColumnDef="statut">
                <th mat-header-cell *matHeaderCellDef>{{ 'orders.table.status' | translate }}</th>
                <td mat-cell *matCellDef="let order">
                  <app-status-badge [status]="order.statut"></app-status-badge>
                </td>
              </ng-container>

              <ng-container matColumnDef="actions">
                <th mat-header-cell *matHeaderCellDef></th>
                <td mat-cell *matCellDef="let order">
                  <a mat-icon-button [routerLink]="[order.id]">
                    <mat-icon>visibility</mat-icon>
                  </a>
                  <a mat-icon-button [routerLink]="[order.id, 'tracking']">
                    <mat-icon>local_shipping</mat-icon>
                  </a>
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="displayedColumns"></tr>
              <tr mat-row *matRowDef="let row; columns: displayedColumns;"
                  class="clickable-row"></tr>
            </table>
          } @else {
            <div class="empty-state">
              <mat-icon>receipt_long</mat-icon>
              <p>{{ 'orders.list.empty' | translate }}</p>
            </div>
          }
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .orders-container {
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

    .filter-bar {
      margin-bottom: 16px;
      overflow-x: auto;
    }

    .full-width-table { width: 100%; }

    .order-link {
      color: var(--faso-primary, #2e7d32);
      text-decoration: none;
      font-weight: 500;
      &:hover { text-decoration: underline; }
    }

    .amount-cell { font-weight: 500; }

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
export class OrdersListComponent implements OnInit {
  readonly activeFilter = signal<FilterStatus>('all');
  readonly orders = signal<Commande[]>([]);

  readonly displayedColumns = ['numero', 'partner', 'quantite', 'date', 'montant', 'statut', 'actions'];

  readonly statusFilters: { value: CommandeStatus; label: string }[] = [
    { value: CommandeStatus.EN_ATTENTE, label: 'orders.filter.pending' },
    { value: CommandeStatus.CONFIRMEE, label: 'orders.filter.confirmed' },
    { value: CommandeStatus.EN_PREPARATION, label: 'orders.filter.preparing' },
    { value: CommandeStatus.EN_LIVRAISON, label: 'orders.filter.ready' },
    { value: CommandeStatus.LIVREE, label: 'orders.filter.delivered' },
    { value: CommandeStatus.ANNULEE, label: 'orders.filter.cancelled' },
  ];

  readonly filteredOrders = computed(() => {
    const filter = this.activeFilter();
    const all = this.orders();
    if (filter === 'all') return all;
    return all.filter(o => o.statut === filter);
  });

  ngOnInit(): void {
    this.loadOrders();
  }

  onFilterChange(value: FilterStatus): void {
    this.activeFilter.set(value);
  }

  countByStatus(status: CommandeStatus): number {
    return this.orders().filter(o => o.statut === status).length;
  }

  getTotalQuantity(order: Commande): number {
    return order.items.reduce((sum, item) => sum + item.quantite, 0);
  }

  private loadOrders(): void {
    this.orders.set([
      {
        id: '1', numero: 'CMD-2026-001', clientId: 'c1', clientNom: 'Restaurant Le Sahel',
        eleveurId: 'e1', eleveurNom: 'Ferme Ouedraogo',
        items: [{ id: 'i1', race: 'Poulet bicyclette', quantite: 50, prixUnitaire: 3500 }],
        statut: CommandeStatus.CONFIRMEE, prixTotal: 175000,
        adresseLivraison: 'Ouagadougou, Secteur 15', telephone: '+226 70 12 34 56',
        createdAt: '2026-04-05',
      },
      {
        id: '2', numero: 'CMD-2026-002', clientId: 'c2', clientNom: 'Mme Traore',
        eleveurId: 'e1', eleveurNom: 'Ferme Ouedraogo',
        items: [{ id: 'i2', race: 'Pintade', quantite: 20, prixUnitaire: 4000 }],
        statut: CommandeStatus.EN_ATTENTE, prixTotal: 80000,
        adresseLivraison: 'Bobo-Dioulasso, Centre', telephone: '+226 76 55 44 33',
        createdAt: '2026-04-06',
      },
      {
        id: '3', numero: 'CMD-2026-003', clientId: 'c3', clientNom: 'Hotel Splendide',
        eleveurId: 'e1', eleveurNom: 'Ferme Ouedraogo',
        items: [{ id: 'i3', race: 'Poulet de chair', quantite: 100, prixUnitaire: 3000 }],
        statut: CommandeStatus.EN_PREPARATION, prixTotal: 300000,
        adresseLivraison: 'Ouagadougou, Zone du Bois', telephone: '+226 25 30 00 00',
        createdAt: '2026-04-03',
      },
      {
        id: '4', numero: 'CMD-2026-004', clientId: 'c4', clientNom: 'M. Kabore',
        eleveurId: 'e1', eleveurNom: 'Ferme Ouedraogo',
        items: [{ id: 'i4', race: 'Poulet bicyclette', quantite: 10, prixUnitaire: 3500 }],
        statut: CommandeStatus.LIVREE, prixTotal: 35000,
        adresseLivraison: 'Koudougou', telephone: '+226 70 99 88 77',
        createdAt: '2026-03-28',
      },
      {
        id: '5', numero: 'CMD-2026-005', clientId: 'c5', clientNom: 'Brasserie du Centre',
        eleveurId: 'e1', eleveurNom: 'Ferme Ouedraogo',
        items: [{ id: 'i5', race: 'Coq local', quantite: 30, prixUnitaire: 5000 }],
        statut: CommandeStatus.ANNULEE, prixTotal: 150000,
        adresseLivraison: 'Ouagadougou, Avenue Kwame', telephone: '+226 25 31 11 11',
        createdAt: '2026-03-25',
      },
    ]);
  }
}
