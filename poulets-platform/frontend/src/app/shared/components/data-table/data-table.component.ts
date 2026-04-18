// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input, TemplateRef, contentChildren, computed, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

export interface TableColumn<T = any> {
  key: string;
  label: string;
  sortable?: boolean;
  width?: string;
  align?: 'left' | 'right' | 'center';
  accessor?: (row: T) => string | number;
}

type SortDirection = 'asc' | 'desc' | null;

@Component({
  selector: 'app-data-table',
  standalone: true,
  imports: [CommonModule, MatIconModule, MatButtonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            @for (col of columns; track col.key) {
              <th
                [class.sortable]="col.sortable"
                [class.align-right]="col.align === 'right'"
                [class.align-center]="col.align === 'center'"
                [style.width]="col.width || null"
                (click)="col.sortable && toggleSort(col.key)"
              >
                {{ col.label }}
                @if (col.sortable) {
                  <mat-icon class="sort-icon">
                    {{ sortKey() === col.key ? (sortDir() === 'asc' ? 'arrow_upward' : sortDir() === 'desc' ? 'arrow_downward' : 'unfold_more') : 'unfold_more' }}
                  </mat-icon>
                }
              </th>
            }
            @if (rowActions) { <th class="align-right">Actions</th> }
          </tr>
        </thead>
        <tbody>
          @if (sortedData().length === 0) {
            <tr>
              <td [attr.colspan]="columns.length + (rowActions ? 1 : 0)" class="empty">
                <mat-icon>inbox</mat-icon>
                {{ emptyMessage }}
              </td>
            </tr>
          } @else {
            @for (row of sortedData(); track rowTrack(row)) {
              <tr>
                @for (col of columns; track col.key) {
                  <td
                    [class.align-right]="col.align === 'right'"
                    [class.align-center]="col.align === 'center'"
                  >
                    {{ getCellValue(row, col) }}
                  </td>
                }
              </tr>
            }
          }
        </tbody>
      </table>
    </div>
  `,
  styles: [`
    :host { display: block; }
    .table-wrap {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      overflow: auto;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--faso-text-sm);
    }
    th {
      background: var(--faso-surface-alt);
      padding: 10px 16px;
      text-align: left;
      color: var(--faso-text-muted);
      text-transform: uppercase;
      font-size: var(--faso-text-xs);
      letter-spacing: 0.04em;
      font-weight: var(--faso-weight-semibold);
      user-select: none;
      white-space: nowrap;
    }
    th.sortable {
      cursor: pointer;
    }
    th.sortable:hover {
      color: var(--faso-text);
    }
    th .sort-icon {
      font-size: 14px;
      width: 14px;
      height: 14px;
      vertical-align: middle;
      margin-left: 2px;
      opacity: 0.7;
    }
    td {
      padding: 12px 16px;
      border-top: 1px solid var(--faso-border);
      vertical-align: top;
      color: var(--faso-text);
    }
    tr:hover td { background: var(--faso-surface-alt); }

    .align-right { text-align: right; }
    .align-center { text-align: center; }

    .empty {
      text-align: center;
      padding: var(--faso-space-10);
      color: var(--faso-text-muted);
    }
    .empty mat-icon {
      display: block;
      margin: 0 auto var(--faso-space-2);
      font-size: 40px;
      width: 40px;
      height: 40px;
      color: var(--faso-text-subtle);
    }
  `],
})
export class DataTableComponent<T = any> {
  @Input({ required: true }) columns: TableColumn<T>[] = [];
  @Input({ required: true }) data: T[] = [];
  @Input() emptyMessage = 'Aucune donnée';
  @Input() rowActions = false;
  @Input() rowKey: (row: T) => string | number = (row) => JSON.stringify(row);

  readonly sortKey = signal<string | null>(null);
  readonly sortDir = signal<SortDirection>(null);

  readonly sortedData = computed(() => {
    const key = this.sortKey();
    const dir = this.sortDir();
    if (!key || !dir) return this.data;
    const col = this.columns.find((c) => c.key === key);
    if (!col) return this.data;
    const acc = col.accessor ?? ((row: T) => (row as any)[key]);
    const arr = [...this.data];
    arr.sort((a, b) => {
      const av = acc(a), bv = acc(b);
      if (av === bv) return 0;
      const cmp = av > bv ? 1 : -1;
      return dir === 'asc' ? cmp : -cmp;
    });
    return arr;
  });

  rowTrack = (row: T): any => this.rowKey(row);

  toggleSort(key: string): void {
    if (this.sortKey() === key) {
      const cycle: SortDirection = this.sortDir() === 'asc' ? 'desc' : this.sortDir() === 'desc' ? null : 'asc';
      this.sortDir.set(cycle);
      if (cycle === null) this.sortKey.set(null);
    } else {
      this.sortKey.set(key);
      this.sortDir.set('asc');
    }
  }

  getCellValue(row: T, col: TableColumn<T>): string | number {
    if (col.accessor) return col.accessor(row);
    const v = (row as any)[col.key];
    return v ?? '—';
  }
}
