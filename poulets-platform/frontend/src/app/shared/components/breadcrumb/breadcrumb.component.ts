// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';

export interface BreadcrumbItem {
  label: string;
  link?: string | any[];
}

@Component({
  selector: 'app-breadcrumb',
  standalone: true,
  imports: [CommonModule, RouterLink, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <nav class="bc" aria-label="Fil d'Ariane">
      <ol>
        @for (item of items; track item.label; let last = $last) {
          <li>
            @if (!last && item.link) {
              <a [routerLink]="item.link">{{ item.label }}</a>
              <mat-icon aria-hidden="true">chevron_right</mat-icon>
            } @else {
              <span [attr.aria-current]="last ? 'page' : null">{{ item.label }}</span>
            }
          </li>
        }
      </ol>
    </nav>
  `,
  styles: [`
    :host { display: block; }
    .bc ol {
      list-style: none;
      padding: 0;
      margin: 0;
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 4px;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }
    li { display: inline-flex; align-items: center; gap: 4px; }
    a {
      color: var(--faso-text-muted);
      text-decoration: none;
    }
    a:hover {
      color: var(--faso-primary-700);
      text-decoration: underline;
    }
    span[aria-current="page"] {
      color: var(--faso-text);
      font-weight: var(--faso-weight-medium);
    }
    mat-icon { font-size: 16px; width: 16px; height: 16px; opacity: 0.6; }
  `],
})
export class BreadcrumbComponent {
  @Input() items: BreadcrumbItem[] = [];
}
