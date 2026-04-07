import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatPaginatorModule, PageEvent } from '@angular/material/paginator';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';

import { PouletService } from '@services/poulet.service';
import { Commande } from '@services/graphql.service';

@Component({
  selector: 'app-client-commandes',
  standalone: true,
  imports: [
    CommonModule,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    MatPaginatorModule,
    MatProgressSpinnerModule,
  ],
  template: `
    <div class="container">
      <div class="page-header">
        <h1>Mes Commandes</h1>
        <p>Historique et suivi de vos commandes</p>
      </div>

      @if (loading()) {
        <div class="loading-overlay">
          <mat-spinner diameter="48"></mat-spinner>
        </div>
      } @else {
        <div class="commandes-list">
          @for (commande of commandes(); track commande.id) {
            <mat-card class="commande-card">
              <mat-card-content>
                <div class="commande-header">
                  <div class="commande-id">
                    <mat-icon>receipt</mat-icon>
                    <span>Commande #{{ commande.id | slice:0:8 }}</span>
                  </div>
                  <mat-chip [class]="'status-' + commande.statut.toLowerCase()">
                    {{ formatStatut(commande.statut) }}
                  </mat-chip>
                </div>

                <div class="commande-body">
                  <div class="commande-poulet">
                    <h3>{{ commande.poulet?.race }}</h3>
                    <p>{{ commande.poulet?.poids }} kg</p>
                  </div>

                  <div class="commande-details">
                    <div class="detail-row">
                      <span>Quantite</span>
                      <strong>{{ commande.quantite }}</strong>
                    </div>
                    <div class="detail-row">
                      <span>Prix unitaire</span>
                      <strong>{{ commande.poulet?.prix | number:'1.0-0' }} FCFA</strong>
                    </div>
                    <div class="detail-row total">
                      <span>Total</span>
                      <strong>{{ commande.prixTotal | number:'1.0-0' }} FCFA</strong>
                    </div>
                  </div>

                  <div class="commande-meta">
                    <span>
                      <mat-icon inline>location_on</mat-icon>
                      {{ commande.adresseLivraison }}
                    </span>
                    <span>
                      <mat-icon inline>calendar_today</mat-icon>
                      {{ commande.createdAt | date:'dd/MM/yyyy HH:mm' }}
                    </span>
                  </div>
                </div>
              </mat-card-content>
            </mat-card>
          } @empty {
            <div class="empty-state">
              <mat-icon>receipt_long</mat-icon>
              <h3>Aucune commande</h3>
              <p>Vous n'avez pas encore passe de commande.</p>
              <a mat-raised-button color="primary" routerLink="/client/catalogue">
                Parcourir le catalogue
              </a>
            </div>
          }
        </div>

        @if (totalElements() > 0) {
          <mat-paginator
            [length]="totalElements()"
            [pageSize]="20"
            [pageIndex]="currentPage()"
            (page)="onPageChange($event)"
            showFirstLastButtons>
          </mat-paginator>
        }
      }
    </div>
  `,
  styles: [`
    .commandes-list {
      display: flex;
      flex-direction: column;
      gap: 16px;
    }

    .commande-card {
      transition: box-shadow 0.2s;

      &:hover {
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
      }
    }

    .commande-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 16px;
    }

    .commande-id {
      display: flex;
      align-items: center;
      gap: 8px;
      font-weight: 500;
      font-size: 0.9rem;
      color: var(--faso-text-secondary);
    }

    .commande-body {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;

      @media (max-width: 600px) {
        grid-template-columns: 1fr;
      }
    }

    .commande-poulet {
      h3 {
        margin: 0 0 4px;
        font-size: 1.1rem;
      }

      p {
        margin: 0;
        color: var(--faso-text-secondary);
        font-size: 0.9rem;
      }
    }

    .commande-details {
      display: flex;
      flex-direction: column;
      gap: 4px;
    }

    .detail-row {
      display: flex;
      justify-content: space-between;
      font-size: 0.9rem;

      &.total {
        margin-top: 4px;
        padding-top: 8px;
        border-top: 1px solid #eee;
        font-size: 1.1rem;
        color: var(--faso-accent-dark);
      }
    }

    .commande-meta {
      grid-column: 1 / -1;
      display: flex;
      gap: 24px;
      padding-top: 12px;
      border-top: 1px solid #eee;
      font-size: 0.85rem;
      color: var(--faso-text-secondary);

      span {
        display: flex;
        align-items: center;
        gap: 4px;
      }
    }

    // Status chip colors
    :host ::ng-deep {
      .status-en_attente { background: #fff3e0; color: #e65100; }
      .status-confirmee { background: #e3f2fd; color: #1565c0; }
      .status-en_livraison { background: #e0f2f1; color: #00695c; }
      .status-livree { background: #e8f5e9; color: #2e7d32; }
      .status-annulee { background: #ffebee; color: #c62828; }
    }

    .empty-state {
      text-align: center;
      padding: 64px 24px;
      color: var(--faso-text-secondary);

      mat-icon {
        font-size: 80px;
        width: 80px;
        height: 80px;
        opacity: 0.3;
      }

      h3 {
        margin: 16px 0 8px;
        font-size: 1.3rem;
      }
    }
  `],
})
export class ClientCommandesComponent implements OnInit {
  private readonly pouletService = inject(PouletService);

  readonly commandes = signal<Commande[]>([]);
  readonly loading = signal(true);
  readonly totalElements = signal(0);
  readonly currentPage = signal(0);

  ngOnInit(): void {
    this.loadCommandes();
  }

  onPageChange(event: PageEvent): void {
    this.currentPage.set(event.pageIndex);
    this.loadCommandes();
  }

  formatStatut(statut: string): string {
    const labels: Record<string, string> = {
      EN_ATTENTE: 'En attente',
      CONFIRMEE: 'Confirmee',
      EN_LIVRAISON: 'En livraison',
      LIVREE: 'Livree',
      ANNULEE: 'Annulee',
    };
    return labels[statut] || statut;
  }

  private loadCommandes(): void {
    this.loading.set(true);
    this.pouletService.getMesCommandes(this.currentPage(), 20).subscribe({
      next: (page) => {
        this.commandes.set(page.content);
        this.totalElements.set(page.totalElements);
        this.loading.set(false);
      },
      error: () => {
        this.loading.set(false);
      },
    });
  }
}
