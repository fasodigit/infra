// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';
import { CommonModule } from '@angular/common';

import type { AdminLevel } from '../models/admin.model';

@Component({
  selector: 'faso-role-chip',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span
      class="fd-chip"
      [class.role-super]="cls() === 'role-super'"
      [class.role-admin]="cls() === 'role-admin'"
      [class.role-manager]="cls() === 'role-manager'"
    >{{ role() }}</span>
  `,
})
export class FasoRoleChipComponent {
  readonly role = input.required<AdminLevel>();

  protected readonly cls = computed<'role-super' | 'role-admin' | 'role-manager'>(() => {
    const r = this.role();
    if (r === 'SUPER-ADMIN') return 'role-super';
    if (r === 'ADMIN') return 'role-admin';
    return 'role-manager';
  });
}
