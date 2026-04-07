import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';
import { CertificationHalal } from '@shared/models/halal.model';

@Component({
  selector: 'app-certification-detail',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatDividerModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
  ],
  template: `
    <div class="detail-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'halal.detail.title' | translate }} {{ cert()?.numero }}</h1>
        <span class="spacer"></span>
        @if (cert(); as c) {
          <app-status-badge [status]="c.statut"></app-status-badge>
        }
      </div>

      @if (cert(); as c) {
        <div class="detail-grid">
          <!-- Certificate Info -->
          <mat-card class="cert-card">
            <mat-card-header>
              <mat-card-title>{{ 'halal.detail.cert_info' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="info-list">
                <div class="info-item">
                  <mat-icon>tag</mat-icon>
                  <div>
                    <span class="label">{{ 'halal.detail.number' | translate }}</span>
                    <span class="value">{{ c.numero }}</span>
                  </div>
                </div>
                <div class="info-item">
                  <mat-icon>business</mat-icon>
                  <div>
                    <span class="label">{{ 'halal.detail.abattoir' | translate }}</span>
                    <span class="value">{{ c.abattoir.nom }}</span>
                    <span class="sub">{{ c.abattoir.adresse }}</span>
                  </div>
                </div>
                <div class="info-item">
                  <mat-icon>event</mat-icon>
                  <div>
                    <span class="label">{{ 'halal.detail.cert_date' | translate }}</span>
                    <span class="value">{{ c.dateCertification | date:'dd MMMM yyyy' }}</span>
                  </div>
                </div>
                <div class="info-item">
                  <mat-icon>event_busy</mat-icon>
                  <div>
                    <span class="label">{{ 'halal.detail.exp_date' | translate }}</span>
                    <span class="value" [class.expired]="isExpired(c.dateExpiration)">
                      {{ c.dateExpiration | date:'dd MMMM yyyy' }}
                    </span>
                  </div>
                </div>
                @if (c.inspecteur) {
                  <div class="info-item">
                    <mat-icon>person</mat-icon>
                    <div>
                      <span class="label">{{ 'halal.detail.imam' | translate }}</span>
                      <span class="value">{{ c.inspecteur }}</span>
                    </div>
                  </div>
                }
                @if (c.lotId) {
                  <div class="info-item">
                    <mat-icon>inventory_2</mat-icon>
                    <div>
                      <span class="label">{{ 'halal.detail.lot_traced' | translate }}</span>
                      <span class="value">{{ c.lotId }}</span>
                    </div>
                  </div>
                }
              </div>
            </mat-card-content>
          </mat-card>

          <!-- QR Code -->
          <mat-card class="qr-card">
            <mat-card-header>
              <mat-card-title>{{ 'halal.detail.qr_code' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="qr-display">
                <!-- SVG placeholder QR code pattern -->
                <svg viewBox="0 0 200 200" class="qr-svg">
                  <rect width="200" height="200" fill="white"/>
                  <!-- Corner markers -->
                  <rect x="10" y="10" width="50" height="50" fill="#1b5e20"/>
                  <rect x="15" y="15" width="40" height="40" fill="white"/>
                  <rect x="20" y="20" width="30" height="30" fill="#1b5e20"/>

                  <rect x="140" y="10" width="50" height="50" fill="#1b5e20"/>
                  <rect x="145" y="15" width="40" height="40" fill="white"/>
                  <rect x="150" y="20" width="30" height="30" fill="#1b5e20"/>

                  <rect x="10" y="140" width="50" height="50" fill="#1b5e20"/>
                  <rect x="15" y="145" width="40" height="40" fill="white"/>
                  <rect x="20" y="150" width="30" height="30" fill="#1b5e20"/>

                  <!-- Data pattern -->
                  @for (row of qrPattern; track $index) {
                    @for (cell of row; track $index) {
                      @if (cell) {
                        <rect [attr.x]="70 + $index * 8" [attr.y]="70 + $index * 8"
                              width="7" height="7" fill="#1b5e20"/>
                      }
                    }
                  }

                  <!-- Decorative data dots -->
                  <rect x="70" y="10" width="8" height="8" fill="#1b5e20"/>
                  <rect x="86" y="10" width="8" height="8" fill="#1b5e20"/>
                  <rect x="102" y="10" width="8" height="8" fill="#1b5e20"/>
                  <rect x="78" y="22" width="8" height="8" fill="#1b5e20"/>
                  <rect x="94" y="22" width="8" height="8" fill="#1b5e20"/>
                  <rect x="110" y="22" width="8" height="8" fill="#1b5e20"/>
                  <rect x="118" y="22" width="8" height="8" fill="#1b5e20"/>
                  <rect x="70" y="70" width="8" height="8" fill="#1b5e20"/>
                  <rect x="86" y="78" width="8" height="8" fill="#1b5e20"/>
                  <rect x="102" y="86" width="8" height="8" fill="#1b5e20"/>
                  <rect x="118" y="94" width="8" height="8" fill="#1b5e20"/>
                  <rect x="134" y="102" width="8" height="8" fill="#1b5e20"/>
                  <rect x="150" y="86" width="8" height="8" fill="#1b5e20"/>
                  <rect x="166" y="70" width="8" height="8" fill="#1b5e20"/>
                </svg>
                <p class="qr-label">{{ c.numero }}</p>
                <button mat-stroked-button color="primary">
                  <mat-icon>download</mat-icon>
                  {{ 'halal.detail.download_qr' | translate }}
                </button>
              </div>
            </mat-card-content>
          </mat-card>
        </div>

        @if (c.observations) {
          <mat-card class="observations-card">
            <mat-card-header>
              <mat-card-title>{{ 'halal.detail.observations' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <p>{{ c.observations }}</p>
            </mat-card-content>
          </mat-card>
        }
      }
    </div>
  `,
  styles: [`
    .detail-container {
      padding: 24px;
      max-width: 1000px;
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

    .detail-grid {
      display: grid;
      grid-template-columns: 2fr 1fr;
      gap: 24px;
      margin-bottom: 24px;
    }

    .info-list {
      display: flex;
      flex-direction: column;
      gap: 16px;
    }

    .info-item {
      display: flex;
      align-items: flex-start;
      gap: 12px;

      mat-icon {
        color: var(--faso-primary, #2e7d32);
        margin-top: 2px;
      }

      div { display: flex; flex-direction: column; }
      .label { font-size: 0.8rem; color: #666; }
      .value { font-weight: 600; }
      .sub { font-size: 0.85rem; color: #888; }
      .expired { color: #f44336; }
    }

    .qr-display {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 12px;
      padding: 16px;
    }

    .qr-svg {
      width: 180px;
      height: 180px;
      border: 1px solid #e0e0e0;
      border-radius: 8px;
    }

    .qr-label {
      font-size: 0.85rem;
      color: #666;
      font-weight: 500;
    }

    .observations-card { margin-top: 24px; }

    @media (max-width: 768px) {
      .detail-grid { grid-template-columns: 1fr; }
    }
  `],
})
export class CertificationDetailComponent implements OnInit {
  readonly cert = signal<CertificationHalal | null>(null);

  readonly qrPattern = [
    [1, 0, 1, 0, 1, 1, 0, 1],
    [0, 1, 0, 1, 0, 0, 1, 0],
    [1, 1, 0, 0, 1, 0, 1, 1],
    [0, 0, 1, 1, 0, 1, 0, 0],
  ];

  constructor(private readonly route: ActivatedRoute) {}

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id');
    this.loadCertification(id!);
  }

  isExpired(dateStr: string): boolean {
    return new Date(dateStr) < new Date();
  }

  private loadCertification(id: string): void {
    this.cert.set({
      id, numero: 'HALAL-2026-001', lotId: 'lot-1',
      abattoir: {
        id: 'ab1', nom: 'Abattoir Moderne de Ouagadougou',
        adresse: 'Zone Industrielle, Ouagadougou', telephone: '+226 25 30 00 00',
        certifie: true, capaciteJournaliere: 500,
      },
      dateCertification: '2026-03-15', dateExpiration: '2026-09-15',
      statut: 'VALIDE', inspecteur: 'Imam Ouedraogo',
      observations: 'Abattage conforme aux normes halal. Lot de 50 poulets Brahma certifies.',
      createdAt: '2026-03-15',
    });
  }
}
