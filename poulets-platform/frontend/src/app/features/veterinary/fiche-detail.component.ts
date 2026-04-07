import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatTableModule } from '@angular/material/table';
import { MatDividerModule } from '@angular/material/divider';
import { MatTabsModule } from '@angular/material/tabs';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';
import { FicheSanitaire, Vaccination, Traitement } from '@shared/models/veterinaire.model';

@Component({
  selector: 'app-fiche-detail',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatTableModule,
    MatDividerModule,
    MatTabsModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
  ],
  template: `
    <div class="fiche-detail-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <div>
          <h1>{{ 'veterinary.detail.title' | translate }} - {{ fiche()?.lotNom }}</h1>
          @if (fiche(); as f) {
            <app-status-badge [status]="f.statut"></app-status-badge>
          }
        </div>
        <span class="spacer"></span>
        <a mat-raised-button color="primary" routerLink="../vaccination/new">
          <mat-icon>vaccines</mat-icon>
          {{ 'veterinary.detail.add_vaccination' | translate }}
        </a>
      </div>

      @if (fiche(); as f) {
        <!-- Lot Info -->
        <div class="info-grid">
          <mat-card>
            <mat-card-content>
              <div class="info-row">
                <mat-icon>inventory_2</mat-icon>
                <div>
                  <span class="info-label">{{ 'veterinary.detail.lot' | translate }}</span>
                  <span class="info-value">{{ f.lotNom }}</span>
                </div>
              </div>
            </mat-card-content>
          </mat-card>
          <mat-card>
            <mat-card-content>
              <div class="info-row">
                <mat-icon>local_hospital</mat-icon>
                <div>
                  <span class="info-label">{{ 'veterinary.detail.vet' | translate }}</span>
                  <span class="info-value">{{ f.veterinaire || '-' }}</span>
                </div>
              </div>
            </mat-card-content>
          </mat-card>
          <mat-card>
            <mat-card-content>
              <div class="info-row">
                <mat-icon>event</mat-icon>
                <div>
                  <span class="info-label">{{ 'veterinary.detail.next_visit' | translate }}</span>
                  <span class="info-value">
                    {{ f.prochaineVisite ? (f.prochaineVisite | date:'dd/MM/yyyy') : '-' }}
                  </span>
                </div>
              </div>
            </mat-card-content>
          </mat-card>
        </div>

        <mat-tab-group>
          <!-- Vaccinations Tab -->
          <mat-tab label="{{ 'veterinary.detail.tab_vaccinations' | translate }}">
            <div class="tab-content">
              <mat-card>
                <mat-card-content>
                  @if (f.vaccinations.length > 0) {
                    <table mat-table [dataSource]="f.vaccinations" class="full-width-table">
                      <ng-container matColumnDef="dateAdministration">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.date' | translate }}</th>
                        <td mat-cell *matCellDef="let v">{{ v.dateAdministration | date:'dd/MM/yyyy' }}</td>
                      </ng-container>
                      <ng-container matColumnDef="nomVaccin">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.vaccine' | translate }}</th>
                        <td mat-cell *matCellDef="let v">{{ v.nomVaccin }}</td>
                      </ng-container>
                      <ng-container matColumnDef="administrePar">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.vet_name' | translate }}</th>
                        <td mat-cell *matCellDef="let v">{{ v.administrePar }}</td>
                      </ng-container>
                      <ng-container matColumnDef="prochaineDose">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.next_dose' | translate }}</th>
                        <td mat-cell *matCellDef="let v">
                          {{ v.prochaineDose ? (v.prochaineDose | date:'dd/MM/yyyy') : '-' }}
                        </td>
                      </ng-container>
                      <ng-container matColumnDef="observations">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.notes' | translate }}</th>
                        <td mat-cell *matCellDef="let v">{{ v.observations || '-' }}</td>
                      </ng-container>
                      <tr mat-header-row *matHeaderRowDef="vaccinColumns"></tr>
                      <tr mat-row *matRowDef="let row; columns: vaccinColumns;"></tr>
                    </table>
                  } @else {
                    <div class="empty-tab">
                      <p>{{ 'veterinary.detail.no_vaccinations' | translate }}</p>
                    </div>
                  }
                </mat-card-content>
              </mat-card>
            </div>
          </mat-tab>

          <!-- Treatments Tab -->
          <mat-tab label="{{ 'veterinary.detail.tab_treatments' | translate }}">
            <div class="tab-content">
              <mat-card>
                <mat-card-content>
                  @if (f.traitements.length > 0) {
                    <table mat-table [dataSource]="f.traitements" class="full-width-table">
                      <ng-container matColumnDef="dateDebut">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.start_date' | translate }}</th>
                        <td mat-cell *matCellDef="let t">{{ t.dateDebut | date:'dd/MM/yyyy' }}</td>
                      </ng-container>
                      <ng-container matColumnDef="diagnostic">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.disease' | translate }}</th>
                        <td mat-cell *matCellDef="let t">{{ t.diagnostic }}</td>
                      </ng-container>
                      <ng-container matColumnDef="nomTraitement">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.treatment' | translate }}</th>
                        <td mat-cell *matCellDef="let t">{{ t.nomTraitement }}</td>
                      </ng-container>
                      <ng-container matColumnDef="duree">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.duration' | translate }}</th>
                        <td mat-cell *matCellDef="let t">{{ t.duree }} {{ 'common.days' | translate }}</td>
                      </ng-container>
                      <ng-container matColumnDef="dateFin">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.end_date' | translate }}</th>
                        <td mat-cell *matCellDef="let t">
                          {{ t.dateFin ? (t.dateFin | date:'dd/MM/yyyy') : '-' }}
                        </td>
                      </ng-container>
                      <ng-container matColumnDef="prescritPar">
                        <th mat-header-cell *matHeaderCellDef>{{ 'veterinary.detail.prescribed_by' | translate }}</th>
                        <td mat-cell *matCellDef="let t">{{ t.prescritPar }}</td>
                      </ng-container>
                      <tr mat-header-row *matHeaderRowDef="treatmentColumns"></tr>
                      <tr mat-row *matRowDef="let row; columns: treatmentColumns;"></tr>
                    </table>
                  } @else {
                    <div class="empty-tab">
                      <p>{{ 'veterinary.detail.no_treatments' | translate }}</p>
                    </div>
                  }
                </mat-card-content>
              </mat-card>
            </div>
          </mat-tab>
        </mat-tab-group>

        <!-- Observations -->
        @if (f.observations) {
          <mat-card class="observations-card">
            <mat-card-header>
              <mat-card-title>{{ 'veterinary.detail.observations' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <p>{{ f.observations }}</p>
            </mat-card-content>
          </mat-card>
        }
      }
    </div>
  `,
  styles: [`
    .fiche-detail-container {
      padding: 24px;
      max-width: 1100px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
      .spacer { flex: 1; }
    }

    .info-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .info-row {
      display: flex;
      align-items: center;
      gap: 12px;

      mat-icon { color: var(--faso-primary, #2e7d32); font-size: 28px; width: 28px; height: 28px; }

      div {
        display: flex;
        flex-direction: column;
      }

      .info-label { font-size: 0.8rem; color: #666; }
      .info-value { font-weight: 600; }
    }

    .tab-content { padding: 16px 0; }

    .full-width-table { width: 100%; }

    .empty-tab {
      text-align: center;
      padding: 32px;
      color: #999;
    }

    .observations-card { margin-top: 24px; }
  `],
})
export class FicheDetailComponent implements OnInit {
  readonly fiche = signal<FicheSanitaire | null>(null);
  readonly vaccinColumns = ['dateAdministration', 'nomVaccin', 'administrePar', 'prochaineDose', 'observations'];
  readonly treatmentColumns = ['dateDebut', 'diagnostic', 'nomTraitement', 'duree', 'dateFin', 'prescritPar'];

  constructor(private readonly route: ActivatedRoute) {}

  ngOnInit(): void {
    const lotId = this.route.snapshot.paramMap.get('lotId');
    this.loadFiche(lotId!);
  }

  private loadFiche(lotId: string): void {
    this.fiche.set({
      id: 'fs1', lotId, lotNom: 'Lot A - Brahma', statut: 'SAIN',
      vaccinations: [
        { id: 'v1', nomVaccin: 'Newcastle (La Sota)', dateAdministration: '2026-02-20', administrePar: 'Dr. Sawadogo', prochaineDose: '2026-05-20', observations: 'Rappel dans 3 mois' },
        { id: 'v2', nomVaccin: 'Gumboro (IBD)', dateAdministration: '2026-03-01', administrePar: 'Dr. Sawadogo' },
        { id: 'v3', nomVaccin: 'Bronchite infectieuse', dateAdministration: '2026-03-15', administrePar: 'Dr. Ouedraogo' },
      ],
      traitements: [
        { id: 't1', nomTraitement: 'Vitamines AD3E', diagnostic: 'Preventif', dateDebut: '2026-03-01', dateFin: '2026-03-05', duree: 5, prescritPar: 'Dr. Sawadogo', observations: 'Renforcement immunitaire' },
      ],
      derniereVisite: '2026-03-28', prochaineVisite: '2026-04-12',
      veterinaire: 'Dr. Sawadogo',
      observations: 'Lot en bonne sante generale. Croissance conforme aux objectifs. Continuer le protocole vaccinal.',
      createdAt: '2026-02-15', updatedAt: '2026-03-28',
    });
  }
}
