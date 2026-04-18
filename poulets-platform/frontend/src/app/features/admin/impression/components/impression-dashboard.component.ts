// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, computed, inject } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { StatCardComponent } from '@shared/components/stat-card/stat-card.component';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { DataTableComponent, TableColumn } from '@shared/components/data-table/data-table.component';
import { ImpressionService, ImpressionJob, JobStatus } from '../services/impression.service';

const STATUS_LABELS: Record<JobStatus, string> = {
  EN_ATTENTE: 'En attente',
  EN_COURS:   'En cours',
  TERMINE:    'Terminé',
  ECHOUE:     'Échoué',
};

@Component({
  selector: 'app-impression-dashboard',
  standalone: true,
  imports: [
    CommonModule, DatePipe, RouterLink, MatIconModule, MatButtonModule,
    StatCardComponent, SectionHeaderComponent, DataTableComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <div>
          <h1>Impression</h1>
          <p>File d'impression des certificats, contrats et récépissés via <code>ec-certificate-renderer</code></p>
        </div>
        <a mat-raised-button color="primary" routerLink="templates">
          <mat-icon>description</mat-icon> Templates disponibles
        </a>
      </header>

      <div class="kpis">
        <app-stat-card icon="hourglass_empty" label="En attente"   [value]="countBy('EN_ATTENTE')" [status]="countBy('EN_ATTENTE') > 5 ? 'degraded' : 'neutral'" />
        <app-stat-card icon="autorenew"       label="En cours"     [value]="countBy('EN_COURS')" status="neutral" />
        <app-stat-card icon="done_all"        label="Terminés"     [value]="countBy('TERMINE')" status="healthy" />
        <app-stat-card icon="error_outline"   label="Échoués"      [value]="countBy('ECHOUE')" [status]="countBy('ECHOUE') > 0 ? 'critical' : 'healthy'" />
      </div>

      <app-section-header title="Derniers jobs" kicker="File d'impression" [linkLabel]="'Archives WORM →'" linkTo="archives" />
      <app-data-table
        [columns]="columns"
        [data]="jobs()"
        [rowKey]="rowKey"
        emptyMessage="Aucun job pour l'instant"
      />
    </section>
  `,
  styles: [`
    :host { display: block; }
    header {
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-5);
      flex-wrap: wrap;
    }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }
    header code {
      font-family: var(--faso-font-mono);
      background: var(--faso-surface-alt);
      padding: 2px 6px;
      border-radius: var(--faso-radius-sm);
      font-size: var(--faso-text-sm);
    }
    .kpis {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: var(--faso-space-4);
      margin-bottom: var(--faso-space-8);
    }
  `],
})
export class ImpressionDashboardComponent implements OnInit {
  private readonly svc = inject(ImpressionService);

  readonly jobs = this.svc.jobs;
  readonly STATUS_LABELS = STATUS_LABELS;

  readonly columns: TableColumn<ImpressionJob>[] = [
    { key: 'id',          label: 'ID',            sortable: true, width: '140px' },
    { key: 'type',        label: 'Type',          sortable: true, accessor: (j) => this.typeLabel(j.type) },
    { key: 'documentId',  label: 'Document' },
    { key: 'status',      label: 'Statut',        sortable: true, accessor: (j) => STATUS_LABELS[j.status] },
    { key: 'attempts',    label: 'Tentatives',    align: 'right' },
    { key: 'requestedAt', label: 'Demandé',       sortable: true, accessor: (j) => new Date(j.requestedAt).toLocaleString('fr-FR') },
    { key: 'completedAt', label: 'Terminé',       accessor: (j) => j.completedAt ? new Date(j.completedAt).toLocaleString('fr-FR') : '—' },
  ];

  rowKey = (j: ImpressionJob) => j.id;

  ngOnInit(): void { this.svc.listJobs().subscribe(); }

  countBy(s: JobStatus): number {
    return this.jobs().filter((j) => j.status === s).length;
  }

  typeLabel(t: string): string {
    switch (t) {
      case 'CERTIFICAT_HALAL':    return 'Certificat halal';
      case 'CONTRAT_COMMANDE':    return 'Contrat commande';
      case 'RECEPISSE_LIVRAISON': return 'Récépissé livraison';
      case 'ATTESTATION_ELEVAGE': return 'Attestation élevage';
    }
    return t;
  }
}
