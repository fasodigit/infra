// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { RouterLink } from '@angular/router';
import { TrustBadgeComponent } from '@shared/components/trust-badge/trust-badge.component';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';

interface Vaccination {
  id: string;
  label: string;
  date: string;
  batch: string;
  vet: string;
}
interface Treatment {
  id: string;
  label: string;
  startDate: string;
  endDate?: string;
  dosage: string;
  reason: string;
  vet: string;
}

@Component({
  selector: 'app-health-record',
  standalone: true,
  imports: [
    CommonModule, DatePipe, RouterLink, MatIconModule, MatButtonModule,
    TrustBadgeComponent, SectionHeaderComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <div class="container">
        <header class="head">
          <div>
            <h1>Dossier sanitaire · Lot {{ lotId() }}</h1>
            <p>Fiche obligatoire pour mise en vente</p>
          </div>
          <div class="badges">
            <app-trust-badge kind="vet" label="Vétérinaire agréé" />
            <app-trust-badge kind="halal" />
          </div>
        </header>

        <div class="grid">
          <article class="card">
            <app-section-header title="Vaccinations" kicker="Obligatoire" />
            <ol class="timeline">
              @for (v of vaccinations(); track v.id) {
                <li>
                  <span class="dot"><mat-icon>vaccines</mat-icon></span>
                  <div>
                    <strong>{{ v.label }}</strong>
                    <time>{{ v.date | date:'mediumDate' }}</time>
                    <span class="meta">Lot {{ v.batch }} · {{ v.vet }}</span>
                  </div>
                </li>
              }
            </ol>
            <button mat-stroked-button class="add" type="button">
              <mat-icon>add</mat-icon> Ajouter une vaccination
            </button>
          </article>

          <article class="card">
            <app-section-header title="Traitements" kicker="Antibiotiques, antiparasitaires" />
            <table>
              <thead>
                <tr>
                  <th scope="col">Traitement</th>
                  <th scope="col">Période</th>
                  <th scope="col">Dosage</th>
                  <th scope="col">Raison</th>
                </tr>
              </thead>
              <tbody>
                @for (t of treatments(); track t.id) {
                  <tr>
                    <td>
                      <strong>{{ t.label }}</strong>
                      <span>{{ t.vet }}</span>
                    </td>
                    <td>
                      {{ t.startDate | date:'shortDate' }}
                      @if (t.endDate) { → {{ t.endDate | date:'shortDate' }} }
                    </td>
                    <td>{{ t.dosage }}</td>
                    <td>{{ t.reason }}</td>
                  </tr>
                }
              </tbody>
            </table>
            <button mat-stroked-button class="add" type="button">
              <mat-icon>add</mat-icon> Ajouter un traitement
            </button>
          </article>
        </div>

        <div class="qrcard">
          <div>
            <h3>Traçabilité publique</h3>
            <p>Partagez le QR code avec vos acheteurs pour qu'ils vérifient l'historique sanitaire.</p>
          </div>
          <div class="qr" aria-label="QR code traçabilité">
            <svg viewBox="0 0 100 100" width="96" height="96">
              <rect width="100" height="100" fill="#FFFFFF"/>
              <g fill="#0F3E1E">
                <rect x="10" y="10" width="25" height="25"/>
                <rect x="65" y="10" width="25" height="25"/>
                <rect x="10" y="65" width="25" height="25"/>
                <rect x="15" y="15" width="15" height="15" fill="#FFFFFF"/>
                <rect x="70" y="15" width="15" height="15" fill="#FFFFFF"/>
                <rect x="15" y="70" width="15" height="15" fill="#FFFFFF"/>
                <rect x="40" y="40" width="6" height="6"/>
                <rect x="50" y="40" width="6" height="6"/>
                <rect x="60" y="50" width="6" height="6"/>
                <rect x="50" y="60" width="6" height="6"/>
                <rect x="40" y="60" width="6" height="6"/>
                <rect x="70" y="50" width="6" height="6"/>
                <rect x="80" y="60" width="6" height="6"/>
                <rect x="40" y="70" width="6" height="6"/>
                <rect x="60" y="70" width="6" height="6"/>
                <rect x="50" y="80" width="6" height="6"/>
                <rect x="70" y="80" width="6" height="6"/>
              </g>
            </svg>
          </div>
        </div>
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .container {
      max-width: 1200px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-12);
    }
    .head {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: var(--faso-space-4);
      margin-bottom: var(--faso-space-6);
      flex-wrap: wrap;
    }
    .head h1 { margin: 0; font-size: var(--faso-text-2xl); font-weight: var(--faso-weight-bold); }
    .head p { margin: 4px 0 0; color: var(--faso-text-muted); }
    .badges { display: flex; gap: 4px; flex-wrap: wrap; }

    .grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--faso-space-5);
      margin-bottom: var(--faso-space-5);
    }
    @media (max-width: 899px) { .grid { grid-template-columns: 1fr; } }

    .card {
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
    }

    .timeline { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--faso-space-3); }
    .timeline li { display: flex; gap: var(--faso-space-3); }
    .timeline .dot {
      width: 36px; height: 36px;
      border-radius: 50%;
      background: var(--faso-success-bg);
      color: var(--faso-success);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }
    .timeline .dot mat-icon { font-size: 18px; width: 18px; height: 18px; }
    .timeline strong { display: block; }
    .timeline time { color: var(--faso-text-muted); font-size: var(--faso-text-sm); }
    .timeline .meta {
      display: block;
      margin-top: 2px;
      color: var(--faso-text-subtle);
      font-size: var(--faso-text-xs);
    }

    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--faso-text-sm);
    }
    th {
      text-align: left;
      padding: 8px 12px;
      background: var(--faso-surface-alt);
      color: var(--faso-text-muted);
      font-weight: var(--faso-weight-semibold);
      text-transform: uppercase;
      font-size: var(--faso-text-xs);
      letter-spacing: 0.04em;
    }
    td {
      padding: 10px 12px;
      border-top: 1px solid var(--faso-border);
      vertical-align: top;
    }
    td strong { display: block; }
    td span { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); }

    .add {
      margin-top: var(--faso-space-3);
      width: 100%;
    }

    .qrcard {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: var(--faso-space-4);
      padding: var(--faso-space-5);
      background: var(--faso-primary-50);
      border: 1px solid var(--faso-primary-200);
      border-radius: var(--faso-radius-xl);
      flex-wrap: wrap;
    }
    .qrcard h3 { margin: 0 0 4px; color: var(--faso-primary-800); }
    .qrcard p { margin: 0; color: var(--faso-text-muted); max-width: 50ch; }
    .qr {
      background: #FFFFFF;
      padding: 8px;
      border-radius: var(--faso-radius-md);
      box-shadow: var(--faso-shadow-sm);
      flex-shrink: 0;
    }
  `],
})
export class HealthRecordComponent {
  readonly lotId = signal('L-2026-041');

  readonly vaccinations = signal<Vaccination[]>([
    { id: 'v1', label: 'Vaccin Newcastle',  date: '2026-02-10', batch: 'NC-2026-09', vet: 'Dr. Compaoré' },
    { id: 'v2', label: 'Vaccin Gumboro',    date: '2026-02-17', batch: 'GB-2026-03', vet: 'Dr. Compaoré' },
    { id: 'v3', label: 'Rappel Newcastle',  date: '2026-03-24', batch: 'NC-2026-12', vet: 'Dr. Bandé' },
  ]);

  readonly treatments = signal<Treatment[]>([
    {
      id: 't1', label: 'Anticoccidien Sulfaquinoxaline',
      startDate: '2026-02-25', endDate: '2026-02-28',
      dosage: '0.2 g/L d\'eau', reason: 'Prévention coccidiose', vet: 'Dr. Compaoré',
    },
    {
      id: 't2', label: 'Vitamines A/D/E',
      startDate: '2026-03-05', endDate: '2026-03-08',
      dosage: '2 ml / 10 L', reason: 'Stress thermique', vet: 'Dr. Bandé',
    },
  ]);
}
