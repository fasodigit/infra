// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, computed, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

import { DataTableComponent, TableColumn } from '@shared/components/data-table/data-table.component';
import { LoadingComponent } from '@shared/components/loading/loading.component';
import { AuditLog } from '@shared/models/admin.models';

const ACTIONS = ['USER_CREATE', 'USER_UPDATE', 'USER_DEACTIVATE', 'LOGIN', 'LOGIN_FAILED', 'MFA_ENROLL', 'ANNONCE_CREATE', 'ANNONCE_DELETE', 'ORDER_CREATE', 'ORDER_CANCEL', 'CONFIG_UPDATE'];
const RESULTS = ['SUCCESS', 'FAILURE', 'DENIED'];

@Component({
  selector: 'app-admin-audit',
  standalone: true,
  imports: [
    CommonModule, DatePipe, FormsModule, MatIconModule, MatButtonModule,
    DataTableComponent, LoadingComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <h1>Logs d'audit</h1>
        <p>Historique complet des actions sur la plateforme</p>
      </header>

      <div class="filters">
        <label class="field">
          <span>Recherche</span>
          <input type="search" [(ngModel)]="search" placeholder="Utilisateur, ressource, IP…">
        </label>
        <label class="field">
          <span>Action</span>
          <select [(ngModel)]="actionFilter">
            <option value="">Toutes</option>
            @for (a of actions; track a) { <option [value]="a">{{ a }}</option> }
          </select>
        </label>
        <label class="field">
          <span>Résultat</span>
          <select [(ngModel)]="resultFilter">
            <option value="">Tous</option>
            @for (r of results; track r) { <option [value]="r">{{ r }}</option> }
          </select>
        </label>
        <label class="field">
          <span>Du</span>
          <input type="date" [(ngModel)]="dateFrom">
        </label>
        <label class="field">
          <span>Au</span>
          <input type="date" [(ngModel)]="dateTo">
        </label>
        <button mat-stroked-button type="button" (click)="exportCsv()">
          <mat-icon>download</mat-icon>
          Export CSV
        </button>
      </div>

      <div class="count">{{ filtered().length }} entrée{{ filtered().length > 1 ? 's' : '' }}</div>

      @if (loading()) {
        <app-loading message="Chargement des logs…" />
      } @else {
        <app-data-table
          [columns]="columns"
          [data]="filtered()"
          emptyMessage="Aucune entrée ne correspond aux filtres"
          [rowKey]="rowKey"
        />
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    header { margin-bottom: var(--faso-space-5); }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .filters {
      display: grid;
      grid-template-columns: 2fr 1fr 1fr 1fr 1fr auto;
      gap: var(--faso-space-3);
      align-items: end;
      padding: var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      margin-bottom: var(--faso-space-3);
    }
    .field { display: flex; flex-direction: column; gap: 4px; }
    .field span {
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      color: var(--faso-text-muted);
      text-transform: uppercase;
      letter-spacing: 0.04em;
    }
    .field input, .field select {
      padding: 8px 12px;
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-md);
      font-family: inherit;
      font-size: var(--faso-text-sm);
      background: var(--faso-surface);
      color: var(--faso-text);
    }
    .field input:focus, .field select:focus {
      outline: none;
      border-color: var(--faso-primary-500);
      box-shadow: 0 0 0 3px var(--faso-primary-100);
    }

    .count {
      padding: var(--faso-space-2) 0;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }

    @media (max-width: 1099px) {
      .filters { grid-template-columns: 1fr 1fr; }
    }
  `],
})
export class AdminAuditComponent implements OnInit {
  readonly loading = signal(false);
  readonly logs = signal<AuditLog[]>([]);

  readonly actions = ACTIONS;
  readonly results = RESULTS;

  search = '';
  actionFilter = '';
  resultFilter = '';
  dateFrom = '';
  dateTo = '';

  readonly columns: TableColumn<AuditLog>[] = [
    { key: 'timestamp', label: 'Horodatage',  sortable: true, width: '180px' },
    { key: 'action',    label: 'Action',      sortable: true, width: '160px' },
    { key: 'user',      label: 'Utilisateur', sortable: true },
    { key: 'resource',  label: 'Ressource' },
    { key: 'ipAddress', label: 'IP',          width: '130px' },
    { key: 'result',    label: 'Résultat',    sortable: true, width: '110px' },
  ];

  rowKey = (r: AuditLog) => r.id;

  readonly filtered = computed(() => {
    const q = this.search.trim().toLowerCase();
    const af = this.actionFilter;
    const rf = this.resultFilter;
    const df = this.dateFrom;
    const dt = this.dateTo;
    return this.logs().filter((l) => {
      if (af && l.action !== af) return false;
      if (rf && l.result !== rf) return false;
      if (df && l.timestamp < df) return false;
      if (dt && l.timestamp > dt + 'T23:59:59') return false;
      if (q) {
        const blob = `${l.user} ${l.resource} ${l.ipAddress} ${l.action} ${l.detail ?? ''}`.toLowerCase();
        if (!blob.includes(q)) return false;
      }
      return true;
    });
  });

  ngOnInit(): void {
    this.loading.set(true);
    // TODO: replace with Apollo query `auditLogs(page, filter)` wired on BFF
    setTimeout(() => {
      this.logs.set(generateMockLogs(80));
      this.loading.set(false);
    }, 300);
  }

  exportCsv(): void {
    const rows = this.filtered();
    const headers = ['timestamp', 'action', 'user', 'userRole', 'resource', 'resourceId', 'ipAddress', 'result', 'detail'];
    const lines = [headers.join(',')];
    for (const r of rows) {
      lines.push(headers.map((h) => csvEscape(String((r as any)[h] ?? ''))).join(','));
    }
    const blob = new Blob([lines.join('\n')], { type: 'text/csv;charset=utf-8' });
    if (typeof window === 'undefined') return;
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `audit-logs-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }
}

function csvEscape(s: string): string {
  if (s.includes(',') || s.includes('"') || s.includes('\n')) {
    return '"' + s.replace(/"/g, '""') + '"';
  }
  return s;
}

function generateMockLogs(n: number): AuditLog[] {
  const users = ['admin@fasodigitalisation.bf', 'kassim.ouedraogo@example.bf', 'awa.sankara@example.bf', 'oumar.traore@example.bf', 'fatim.compaore@example.bf'];
  const resources = ['/users/42', '/marketplace/annonces/a-7', '/orders/CMD-ABC12', '/platform-config/flags', '/auth/login', '/profile/mfa'];
  const out: AuditLog[] = [];
  for (let i = 0; i < n; i++) {
    const action = ACTIONS[i % ACTIONS.length]!;
    const resource = resources[i % resources.length]!;
    const user = users[i % users.length]!;
    const result = (Math.random() < 0.85 ? 'SUCCESS' : Math.random() < 0.5 ? 'FAILURE' : 'DENIED') as any;
    out.push({
      id: 'log-' + i,
      timestamp: new Date(Date.now() - i * 90 * 60000).toISOString(),
      action,
      user,
      resource,
      ipAddress: `192.168.${Math.floor(Math.random() * 255)}.${Math.floor(Math.random() * 255)}`,
      result,
    });
  }
  return out;
}
