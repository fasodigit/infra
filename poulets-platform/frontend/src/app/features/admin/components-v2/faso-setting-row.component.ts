// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, input } from '@angular/core';
import { CommonModule } from '@angular/common';

/**
 * Ligne de configuration de la page Settings.
 *
 * - colonne gauche : clé monospace + label + description
 * - colonne droite : `<ng-content/>` (le contrôle injecté par le parent)
 *   suivi de la méta `vN · maj par X`.
 */
@Component({
  selector: 'faso-setting-row',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="fd-setting-row" [class.dirty]="dirty()">
      <div>
        <div class="fd-setting-key">{{ k() }}</div>
        <div class="fd-setting-label">{{ label() }}</div>
        @if (desc()) {
          <div class="fd-setting-desc">{{ desc() }}</div>
        }
      </div>
      <div class="fd-setting-control">
        <ng-content/>
        <div class="fd-setting-meta">
          <span>v{{ version() }}</span>
          @if (updatedBy()) {
            <span>· {{ updatedBy() }}</span>
          }
        </div>
      </div>
    </div>
  `,
})
export class FasoSettingRowComponent {
  readonly k = input.required<string>();
  readonly label = input.required<string>();
  readonly desc = input<string>('');
  readonly version = input<number | string>(1);
  readonly updatedBy = input<string>('');
  readonly dirty = input<boolean>(false);
}
