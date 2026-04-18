// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { DataTableComponent, TableColumn } from '@shared/components/data-table/data-table.component';
import { LoadingComponent } from '@shared/components/loading/loading.component';
import { ImpressionService, WormArchive } from '../services/impression.service';

@Component({
  selector: 'app-worm-archives',
  standalone: true,
  imports: [CommonModule, DatePipe, RouterLink, MatIconModule, MatButtonModule, DataTableComponent, LoadingComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <a mat-button routerLink=".." class="back">
          <mat-icon>arrow_back</mat-icon> Retour
        </a>
        <div>
          <h1>Archives WORM</h1>
          <p>Documents immuables scellés avec QR code de vérification (Write-Once-Read-Many)</p>
        </div>
      </header>

      @if (loading()) {
        <app-loading message="Chargement des archives…" />
      } @else {
        <app-data-table
          [columns]="columns"
          [data]="archives()"
          [rowKey]="rowKey"
          emptyMessage="Aucune archive WORM"
        />
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    .back { margin-left: calc(var(--faso-space-4) * -1); color: var(--faso-text-muted); margin-bottom: var(--faso-space-2); display: inline-flex; }
    header { margin-bottom: var(--faso-space-5); }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }
  `],
})
export class WormArchivesComponent implements OnInit {
  private readonly svc = inject(ImpressionService);

  readonly archives = signal<WormArchive[]>([]);
  readonly loading = signal(true);

  readonly columns: TableColumn<WormArchive>[] = [
    { key: 'id',           label: 'ID archive',   width: '140px' },
    { key: 'type',         label: 'Type',         sortable: true },
    { key: 'documentId',   label: 'Document' },
    { key: 'sha256',       label: 'SHA-256',      accessor: (a) => a.sha256.slice(0, 12) + '…' },
    { key: 'sealedAt',     label: 'Scellé',       sortable: true, accessor: (a) => new Date(a.sealedAt).toLocaleString('fr-FR') },
    { key: 'qrVerificationUrl', label: 'Vérification', accessor: (a) => a.qrVerificationUrl },
  ];

  rowKey = (a: WormArchive) => a.id;

  ngOnInit(): void {
    this.svc.listArchives().subscribe({
      next: (arr) => { this.archives.set(arr); this.loading.set(false); },
      error: () => this.loading.set(false),
    });
  }
}
