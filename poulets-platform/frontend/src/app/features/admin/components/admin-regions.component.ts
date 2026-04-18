// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, signal } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { DataTableComponent, TableColumn } from '@shared/components/data-table/data-table.component';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { StatCardComponent } from '@shared/components/stat-card/stat-card.component';

interface RegionRow {
  code: string;
  name: string;
  capital: string;
  eleveurs: number;
  clients: number;
  orders: number;
  gmvFcfa: number;
  moderator?: string;
  halalPct: number;
}

@Component({
  selector: 'app-admin-regions',
  standalone: true,
  imports: [CommonModule, DecimalPipe, MatIconModule, MatButtonModule, DataTableComponent, SectionHeaderComponent, StatCardComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <h1>Régions · Burkina Faso</h1>
        <p>13 régions · couverture plateforme en temps réel</p>
      </header>

      <div class="kpis">
        <app-stat-card icon="map" label="Régions actives" [value]="activeRegions()" sublabel="sur 13"/>
        <app-stat-card icon="person" label="Éleveurs (total)" [value]="totalEleveurs()" sublabel="dans toutes les régions"/>
        <app-stat-card icon="receipt_long" label="Commandes 7j" [value]="totalOrders()" sublabel="toutes régions confondues"/>
        <app-stat-card icon="payments" label="GMV 7j" [value]="gmvM()" unit="M FCFA" sublabel="toutes régions"/>
      </div>

      <app-section-header title="Détail par région" kicker="13 régions BF" />
      <app-data-table
        [columns]="columns"
        [data]="rows()"
        [rowKey]="rowKey"
        emptyMessage="Aucune région"
      />
    </section>
  `,
  styles: [`
    :host { display: block; }
    header { margin-bottom: var(--faso-space-5); }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .kpis {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: var(--faso-space-4);
      margin-bottom: var(--faso-space-8);
    }
  `],
})
export class AdminRegionsComponent {
  readonly rows = signal<RegionRow[]>([
    { code: 'BMH', name: 'Boucle du Mouhoun', capital: 'Dédougou',        eleveurs: 18, clients: 62,  orders: 8,  gmvFcfa: 360000,  moderator: 'Aminata Y.',   halalPct: 72 },
    { code: 'CAS', name: 'Cascades',           capital: 'Banfora',          eleveurs: 9,  clients: 34,  orders: 5,  gmvFcfa: 180000,  moderator: 'Aminata Y.',   halalPct: 55 },
    { code: 'CEN', name: 'Centre',             capital: 'Ouagadougou',     eleveurs: 72, clients: 410, orders: 38, gmvFcfa: 2140000, moderator: 'Admin FASO',  halalPct: 89 },
    { code: 'CNE', name: 'Centre-Est',         capital: 'Tenkodogo',        eleveurs: 14, clients: 48,  orders: 3,  gmvFcfa: 140000,  moderator: 'Aminata Y.',   halalPct: 60 },
    { code: 'CNO', name: 'Centre-Nord',        capital: 'Kaya',             eleveurs: 21, clients: 58,  orders: 6,  gmvFcfa: 280000,  moderator: 'Admin FASO',  halalPct: 68 },
    { code: 'COU', name: 'Centre-Ouest',       capital: 'Koudougou',        eleveurs: 31, clients: 128, orders: 14, gmvFcfa: 720000,  moderator: 'Aminata Y.',   halalPct: 78 },
    { code: 'CSU', name: 'Centre-Sud',         capital: 'Manga',            eleveurs: 11, clients: 41,  orders: 3,  gmvFcfa: 140000,  moderator: '—',            halalPct: 48 },
    { code: 'EST', name: 'Est',                 capital: 'Fada N\'gourma',  eleveurs: 12, clients: 34,  orders: 2,  gmvFcfa: 96000,   moderator: '—',            halalPct: 52 },
    { code: 'HBS', name: 'Hauts-Bassins',      capital: 'Bobo-Dioulasso',  eleveurs: 58, clients: 302, orders: 29, gmvFcfa: 1620000, moderator: 'Admin FASO',  halalPct: 85 },
    { code: 'NRD', name: 'Nord',                capital: 'Ouahigouya',      eleveurs: 15, clients: 47,  orders: 4,  gmvFcfa: 190000,  moderator: 'Aminata Y.',   halalPct: 62 },
    { code: 'PCE', name: 'Plateau-Central',    capital: 'Ziniaré',          eleveurs: 9,  clients: 28,  orders: 2,  gmvFcfa: 82000,   moderator: 'Admin FASO',  halalPct: 50 },
    { code: 'SAH', name: 'Sahel',               capital: 'Dori',             eleveurs: 6,  clients: 18,  orders: 1,  gmvFcfa: 42000,   moderator: '—',            halalPct: 38 },
    { code: 'SOU', name: 'Sud-Ouest',           capital: 'Gaoua',            eleveurs: 7,  clients: 22,  orders: 1,  gmvFcfa: 58000,   moderator: '—',            halalPct: 45 },
  ]);

  readonly columns: TableColumn<RegionRow>[] = [
    { key: 'name',      label: 'Région',     sortable: true },
    { key: 'capital',   label: 'Capitale' },
    { key: 'eleveurs',  label: 'Éleveurs',   sortable: true, align: 'right' },
    { key: 'clients',   label: 'Clients',    sortable: true, align: 'right' },
    { key: 'orders',    label: 'Commandes 7j', sortable: true, align: 'right' },
    { key: 'gmvFcfa',   label: 'GMV (FCFA)', sortable: true, align: 'right', accessor: (r) => r.gmvFcfa.toLocaleString('fr-FR') },
    { key: 'halalPct',  label: '% Halal',    sortable: true, align: 'right', accessor: (r) => r.halalPct + '%' },
    { key: 'moderator', label: 'Modérateur' },
  ];

  rowKey = (r: RegionRow) => r.code;

  activeRegions = () => this.rows().filter((r) => r.eleveurs > 0).length;
  totalEleveurs = () => this.rows().reduce((s, r) => s + r.eleveurs, 0);
  totalOrders = () => this.rows().reduce((s, r) => s + r.orders, 0);
  gmvM = () => (this.rows().reduce((s, r) => s + r.gmvFcfa, 0) / 1_000_000).toFixed(2);
}
