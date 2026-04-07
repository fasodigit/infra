import { Component, OnInit, inject, signal, computed } from '@angular/core';
import { CommonModule, DecimalPipe, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatTableModule } from '@angular/material/table';
import { MatChipsModule } from '@angular/material/chips';
import { MatListModule } from '@angular/material/list';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';
import { AuthService } from '../../core/services/auth.service';

interface Delivery {
  date: string;
  eleveur: string;
  quantite: number;
  heure: string;
}

interface RecentOrder {
  id: string;
  eleveur: string;
  race: string;
  quantite: number;
  prixTotal: number;
  statut: string;
  date: string;
}

interface Recommendation {
  id: string;
  nom: string;
  localisation: string;
  note: number;
  races: string[];
}

@Component({
  selector: 'app-client-dashboard',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatTableModule,
    MatChipsModule,
    MatListModule,
    MatDividerModule,
    TranslateModule,
    DecimalPipe,
    DatePipe,
  ],
  template: `
    <div class="dashboard-container">
      <div class="dashboard-header">
        <h1>{{ 'dashboard.title' | translate }}</h1>
        <p class="welcome-text">{{ 'dashboard.welcome' | translate:{ name: userName() } }}</p>
      </div>

      <!-- KPI Cards -->
      <div class="kpi-grid">
        <mat-card class="kpi-card" style="border-left-color: #4caf50">
          <mat-card-content>
            <div class="kpi-content">
              <div class="kpi-info">
                <span class="kpi-label">Besoins actifs</span>
                <span class="kpi-value">3</span>
              </div>
              <mat-icon class="kpi-icon" style="color: #4caf50">assignment</mat-icon>
            </div>
          </mat-card-content>
        </mat-card>

        <mat-card class="kpi-card" style="border-left-color: #ff9800">
          <mat-card-content>
            <div class="kpi-content">
              <div class="kpi-info">
                <span class="kpi-label">{{ 'dashboard.pending_orders' | translate }}</span>
                <span class="kpi-value">{{ pendingOrders() }}</span>
              </div>
              <mat-icon class="kpi-icon" style="color: #ff9800">shopping_cart</mat-icon>
            </div>
          </mat-card-content>
        </mat-card>

        <mat-card class="kpi-card" style="border-left-color: #9c27b0">
          <mat-card-content>
            <div class="kpi-content">
              <div class="kpi-info">
                <span class="kpi-label">Depenses du mois</span>
                <span class="kpi-value">{{ monthExpenses() | number:'1.0-0' }} FCFA</span>
              </div>
              <mat-icon class="kpi-icon" style="color: #9c27b0">account_balance_wallet</mat-icon>
            </div>
          </mat-card-content>
        </mat-card>

        <mat-card class="kpi-card" style="border-left-color: #e91e63">
          <mat-card-content>
            <div class="kpi-content">
              <div class="kpi-info">
                <span class="kpi-label">Eleveurs favoris</span>
                <span class="kpi-value">{{ favoriteEleveurs() }}</span>
              </div>
              <mat-icon class="kpi-icon" style="color: #e91e63">favorite</mat-icon>
            </div>
          </mat-card-content>
        </mat-card>
      </div>

      <div class="content-row">
        <!-- Calendar Widget: Deliveries This Week -->
        <mat-card class="calendar-card">
          <mat-card-header>
            <mat-card-title>
              <mat-icon>event</mat-icon>
              Prochaines livraisons cette semaine
            </mat-card-title>
          </mat-card-header>
          <mat-card-content>
            <mat-list>
              @for (delivery of upcomingDeliveries(); track delivery.date + delivery.eleveur) {
                <mat-list-item>
                  <mat-icon matListItemIcon>local_shipping</mat-icon>
                  <div matListItemTitle>
                    {{ delivery.eleveur }} - {{ delivery.quantite }} poulets
                  </div>
                  <div matListItemLine>
                    {{ delivery.date | date:'EEEE dd MMM' }} a {{ delivery.heure }}
                  </div>
                </mat-list-item>
                <mat-divider></mat-divider>
              } @empty {
                <div class="empty-state">
                  <mat-icon>event_busy</mat-icon>
                  <p>{{ 'calendar.no_deliveries' | translate }}</p>
                </div>
              }
            </mat-list>
          </mat-card-content>
        </mat-card>

        <!-- Recommendations -->
        <mat-card class="recommendations-card">
          <mat-card-header>
            <mat-card-title>
              <mat-icon>recommend</mat-icon>
              Eleveurs correspondant a vos besoins
            </mat-card-title>
          </mat-card-header>
          <mat-card-content>
            @for (rec of recommendations(); track rec.id) {
              <div class="recommendation-item">
                <div class="rec-avatar">
                  <mat-icon>person</mat-icon>
                </div>
                <div class="rec-info">
                  <strong>{{ rec.nom }}</strong>
                  <span class="rec-location">
                    <mat-icon inline>location_on</mat-icon> {{ rec.localisation }}
                  </span>
                  <div class="rec-races">
                    @for (race of rec.races; track race) {
                      <mat-chip>{{ race }}</mat-chip>
                    }
                  </div>
                </div>
                <div class="rec-rating">
                  <mat-icon>star</mat-icon>
                  {{ rec.note }}
                </div>
              </div>
            }
          </mat-card-content>
        </mat-card>
      </div>

      <!-- Recent Orders Table -->
      <mat-card class="table-card">
        <mat-card-header>
          <mat-card-title>{{ 'dashboard.recent_orders' | translate }}</mat-card-title>
          <span class="spacer"></span>
          <a mat-button color="primary" routerLink="/orders">
            {{ 'common.view_all' | translate }}
          </a>
        </mat-card-header>
        <mat-card-content>
          <table mat-table [dataSource]="recentOrders()" class="full-width-table">
            <ng-container matColumnDef="eleveur">
              <th mat-header-cell *matHeaderCellDef>Eleveur</th>
              <td mat-cell *matCellDef="let order">{{ order.eleveur }}</td>
            </ng-container>
            <ng-container matColumnDef="race">
              <th mat-header-cell *matHeaderCellDef>Race</th>
              <td mat-cell *matCellDef="let order">{{ order.race }}</td>
            </ng-container>
            <ng-container matColumnDef="quantite">
              <th mat-header-cell *matHeaderCellDef>{{ 'contracts.quantity' | translate }}</th>
              <td mat-cell *matCellDef="let order">{{ order.quantite }}</td>
            </ng-container>
            <ng-container matColumnDef="prixTotal">
              <th mat-header-cell *matHeaderCellDef>{{ 'orders.total' | translate }}</th>
              <td mat-cell *matCellDef="let order">{{ order.prixTotal | number:'1.0-0' }} FCFA</td>
            </ng-container>
            <ng-container matColumnDef="statut">
              <th mat-header-cell *matHeaderCellDef>{{ 'orders.status' | translate }}</th>
              <td mat-cell *matCellDef="let order">
                <mat-chip>{{ order.statut }}</mat-chip>
              </td>
            </ng-container>
            <tr mat-header-row *matHeaderRowDef="orderColumns"></tr>
            <tr mat-row *matRowDef="let row; columns: orderColumns;"></tr>
          </table>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .dashboard-container {
      padding: 24px;
      max-width: 1400px;
      margin: 0 auto;
    }

    .dashboard-header {
      margin-bottom: 24px;

      h1 { margin: 0; font-size: 1.8rem; color: var(--faso-primary-dark, #1b5e20); }
      .welcome-text { color: #666; margin: 4px 0 0; }
    }

    .kpi-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .kpi-card {
      border-left: 4px solid;

      .kpi-content { display: flex; justify-content: space-between; align-items: center; }
      .kpi-info { display: flex; flex-direction: column; }
      .kpi-label { font-size: 0.85rem; color: #666; margin-bottom: 4px; }
      .kpi-value { font-size: 1.5rem; font-weight: 700; color: #333; }
      .kpi-icon { font-size: 40px; width: 40px; height: 40px; opacity: 0.7; }
    }

    .content-row {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .calendar-card, .recommendations-card {
      mat-card-header mat-card-title {
        display: flex;
        align-items: center;
        gap: 8px;
      }
    }

    .empty-state {
      text-align: center;
      padding: 32px;
      color: #999;

      mat-icon { font-size: 48px; width: 48px; height: 48px; opacity: 0.4; }
    }

    .recommendation-item {
      display: flex;
      align-items: center;
      gap: 16px;
      padding: 12px 0;
      border-bottom: 1px solid #eee;

      &:last-child { border-bottom: none; }
    }

    .rec-avatar {
      width: 48px;
      height: 48px;
      border-radius: 50%;
      background: #e8f5e9;
      display: flex;
      align-items: center;
      justify-content: center;

      mat-icon { color: #4caf50; }
    }

    .rec-info {
      flex: 1;

      .rec-location {
        display: flex;
        align-items: center;
        gap: 2px;
        font-size: 0.85rem;
        color: #666;
      }

      .rec-races {
        display: flex;
        gap: 4px;
        margin-top: 4px;
        flex-wrap: wrap;
      }
    }

    .rec-rating {
      display: flex;
      align-items: center;
      gap: 4px;
      font-weight: 600;
      color: #ff9800;
    }

    .table-card {
      mat-card-header {
        display: flex;
        align-items: center;
        .spacer { flex: 1; }
      }
    }

    .full-width-table { width: 100%; }
  `],
})
export class ClientDashboardComponent implements OnInit {
  private readonly auth = inject(AuthService);

  readonly userName = computed(() => this.auth.currentUser()?.nom ?? 'Client');
  readonly pendingOrders = signal(4);
  readonly monthExpenses = signal(285000);
  readonly favoriteEleveurs = signal(7);

  readonly upcomingDeliveries = signal<Delivery[]>([]);
  readonly recentOrders = signal<RecentOrder[]>([]);
  readonly recommendations = signal<Recommendation[]>([]);

  readonly orderColumns = ['eleveur', 'race', 'quantite', 'prixTotal', 'statut'];

  ngOnInit(): void {
    this.loadDeliveries();
    this.loadRecentOrders();
    this.loadRecommendations();
  }

  private loadDeliveries(): void {
    this.upcomingDeliveries.set([
      { date: '2026-04-08', eleveur: 'Ferme Ouedraogo', quantite: 20, heure: '09:00' },
      { date: '2026-04-09', eleveur: 'Elevage Kabore', quantite: 15, heure: '14:30' },
      { date: '2026-04-11', eleveur: 'Cooperative Sahel', quantite: 50, heure: '08:00' },
    ]);
  }

  private loadRecentOrders(): void {
    this.recentOrders.set([
      { id: 'CMD-101', eleveur: 'Ferme Ouedraogo', race: 'Brahma', quantite: 20, prixTotal: 120000, statut: 'Confirmee', date: '2026-04-05' },
      { id: 'CMD-102', eleveur: 'Elevage Kabore', race: 'Sussex', quantite: 15, prixTotal: 82500, statut: 'En livraison', date: '2026-04-03' },
      { id: 'CMD-103', eleveur: 'Coop. du Centre', race: 'Race locale', quantite: 30, prixTotal: 135000, statut: 'En attente', date: '2026-04-01' },
    ]);
  }

  private loadRecommendations(): void {
    this.recommendations.set([
      { id: '1', nom: 'Ferme Bio Ouaga', localisation: 'Ouagadougou', note: 4.8, races: ['Brahma', 'Sussex'] },
      { id: '2', nom: 'Elevage Saaba', localisation: 'Saaba', note: 4.6, races: ['Race locale', 'Pintade'] },
      { id: '3', nom: 'Coop Poulets Frais', localisation: 'Bobo-Dioulasso', note: 4.5, races: ['Leghorn', 'Coucou'] },
    ]);
  }
}
