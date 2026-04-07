import { Component, OnInit, signal, computed } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatTableModule } from '@angular/material/table';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatTabsModule } from '@angular/material/tabs';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';
import { Livraison, ModeLivraison } from '@shared/models/livraison.model';

@Component({
  selector: 'app-deliveries-list',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatTableModule,
    MatButtonModule,
    MatIconModule,
    MatTabsModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
  ],
  template: `
    <div class="deliveries-container">
      <div class="page-header">
        <h1>{{ 'delivery.list.title' | translate }}</h1>
      </div>

      <mat-tab-group (selectedTabChange)="onTabChange($event.index)">
        <!-- Upcoming -->
        <mat-tab label="{{ 'delivery.list.upcoming' | translate }}">
          <div class="tab-content">
            @if (upcomingDeliveries().length > 0) {
              <mat-card>
                <mat-card-content>
                  <table mat-table [dataSource]="upcomingDeliveries()" class="full-width-table">
                    <ng-container matColumnDef="from">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.from' | translate }}</th>
                      <td mat-cell *matCellDef="let d">{{ d.adresseDepart }}</td>
                    </ng-container>

                    <ng-container matColumnDef="to">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.to' | translate }}</th>
                      <td mat-cell *matCellDef="let d">{{ d.adresseArrivee }}</td>
                    </ng-container>

                    <ng-container matColumnDef="date">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.date' | translate }}</th>
                      <td mat-cell *matCellDef="let d">{{ d.dateEstimee | date:'dd/MM/yyyy' }}</td>
                    </ng-container>

                    <ng-container matColumnDef="mode">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.mode' | translate }}</th>
                      <td mat-cell *matCellDef="let d">
                        <span class="mode-badge">
                          <mat-icon>{{ getModeIcon(d.modeLivraison) }}</mat-icon>
                          {{ d.modeLivraison }}
                        </span>
                      </td>
                    </ng-container>

                    <ng-container matColumnDef="statut">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.status' | translate }}</th>
                      <td mat-cell *matCellDef="let d">
                        <app-status-badge [status]="d.statut"></app-status-badge>
                      </td>
                    </ng-container>

                    <ng-container matColumnDef="actions">
                      <th mat-header-cell *matHeaderCellDef></th>
                      <td mat-cell *matCellDef="let d">
                        <a mat-icon-button [routerLink]="[d.id]">
                          <mat-icon>visibility</mat-icon>
                        </a>
                      </td>
                    </ng-container>

                    <tr mat-header-row *matHeaderRowDef="displayedColumns"></tr>
                    <tr mat-row *matRowDef="let row; columns: displayedColumns;" class="clickable-row"></tr>
                  </table>
                </mat-card-content>
              </mat-card>
            } @else {
              <div class="empty-state">
                <mat-icon>local_shipping</mat-icon>
                <p>{{ 'delivery.list.no_upcoming' | translate }}</p>
              </div>
            }
          </div>
        </mat-tab>

        <!-- Past -->
        <mat-tab label="{{ 'delivery.list.past' | translate }}">
          <div class="tab-content">
            @if (pastDeliveries().length > 0) {
              <mat-card>
                <mat-card-content>
                  <table mat-table [dataSource]="pastDeliveries()" class="full-width-table">
                    <ng-container matColumnDef="from">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.from' | translate }}</th>
                      <td mat-cell *matCellDef="let d">{{ d.adresseDepart }}</td>
                    </ng-container>

                    <ng-container matColumnDef="to">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.to' | translate }}</th>
                      <td mat-cell *matCellDef="let d">{{ d.adresseArrivee }}</td>
                    </ng-container>

                    <ng-container matColumnDef="date">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.date' | translate }}</th>
                      <td mat-cell *matCellDef="let d">{{ d.dateLivraison | date:'dd/MM/yyyy' }}</td>
                    </ng-container>

                    <ng-container matColumnDef="mode">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.mode' | translate }}</th>
                      <td mat-cell *matCellDef="let d">
                        <span class="mode-badge">
                          <mat-icon>{{ getModeIcon(d.modeLivraison) }}</mat-icon>
                          {{ d.modeLivraison }}
                        </span>
                      </td>
                    </ng-container>

                    <ng-container matColumnDef="statut">
                      <th mat-header-cell *matHeaderCellDef>{{ 'delivery.table.status' | translate }}</th>
                      <td mat-cell *matCellDef="let d">
                        <app-status-badge [status]="d.statut"></app-status-badge>
                      </td>
                    </ng-container>

                    <ng-container matColumnDef="actions">
                      <th mat-header-cell *matHeaderCellDef></th>
                      <td mat-cell *matCellDef="let d">
                        <a mat-icon-button [routerLink]="[d.id]">
                          <mat-icon>visibility</mat-icon>
                        </a>
                      </td>
                    </ng-container>

                    <tr mat-header-row *matHeaderRowDef="displayedColumns"></tr>
                    <tr mat-row *matRowDef="let row; columns: displayedColumns;" class="clickable-row"></tr>
                  </table>
                </mat-card-content>
              </mat-card>
            } @else {
              <div class="empty-state">
                <mat-icon>history</mat-icon>
                <p>{{ 'delivery.list.no_past' | translate }}</p>
              </div>
            }
          </div>
        </mat-tab>
      </mat-tab-group>
    </div>
  `,
  styles: [`
    .deliveries-container {
      padding: 24px;
      max-width: 1200px;
      margin: 0 auto;
    }

    .page-header {
      margin-bottom: 24px;
      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .tab-content { padding: 16px 0; }

    .full-width-table { width: 100%; }

    .mode-badge {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      font-size: 0.85rem;

      mat-icon { font-size: 18px; width: 18px; height: 18px; color: #666; }
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
export class DeliveriesListComponent implements OnInit {
  readonly deliveries = signal<Livraison[]>([]);
  readonly displayedColumns = ['from', 'to', 'date', 'mode', 'statut', 'actions'];

  readonly upcomingDeliveries = computed(() =>
    this.deliveries().filter(d => d.statut === 'PLANIFIEE' || d.statut === 'EN_COURS')
  );

  readonly pastDeliveries = computed(() =>
    this.deliveries().filter(d => d.statut === 'LIVREE' || d.statut === 'ECHOUEE' || d.statut === 'ANNULEE')
  );

  ngOnInit(): void {
    this.loadDeliveries();
  }

  onTabChange(index: number): void {
    // Tab switched - data is already computed
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

  private loadDeliveries(): void {
    this.deliveries.set([
      {
        id: 'liv-1', commandeId: 'cmd-1', modeLivraison: ModeLivraison.MOTO,
        adresseDepart: 'Ferme Ouedraogo, Koudougou', adresseArrivee: 'Restaurant Le Sahel, Ouagadougou',
        dateEstimee: '2026-04-10', statut: 'PLANIFIEE',
        livreur: { id: 'l1', nom: 'Ibrahim Kabore', telephone: '+226 70 11 22 33', modeLivraison: ModeLivraison.MOTO },
        createdAt: '2026-04-05',
      },
      {
        id: 'liv-2', commandeId: 'cmd-2', modeLivraison: ModeLivraison.RETRAIT,
        adresseDepart: 'Ferme Ouedraogo, Koudougou', adresseArrivee: 'Retrait sur place',
        dateEstimee: '2026-04-08', statut: 'EN_COURS',
        createdAt: '2026-04-06',
      },
      {
        id: 'liv-3', commandeId: 'cmd-3', modeLivraison: ModeLivraison.CAMION,
        adresseDepart: 'Ferme Ouedraogo, Koudougou', adresseArrivee: 'Hotel Splendide, Ouagadougou',
        dateEstimee: '2026-04-03', dateLivraison: '2026-04-03', statut: 'LIVREE',
        createdAt: '2026-04-01',
      },
      {
        id: 'liv-4', commandeId: 'cmd-4', modeLivraison: ModeLivraison.VOITURE,
        adresseDepart: 'Ferme Ouedraogo, Koudougou', adresseArrivee: 'M. Kabore, Koudougou',
        dateEstimee: '2026-03-28', dateLivraison: '2026-03-28', statut: 'LIVREE',
        createdAt: '2026-03-26',
      },
    ]);
  }
}
