// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';
import { CommonModule } from '@angular/common';

import type { AdminUser } from '../models/admin.model';

type AvatarUser = Pick<AdminUser, 'firstName' | 'lastName' | 'avatar'>;

@Component({
  selector: 'faso-avatar',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span
      class="fd-avatar"
      [style.width.px]="size()"
      [style.height.px]="size()"
      [style.fontSize.px]="size() * 0.36"
      [style.background]="user().avatar || '#1b5e20'"
    >{{ initials() }}</span>
  `,
})
export class FasoAvatarComponent {
  readonly user = input.required<AvatarUser>();
  readonly size = input<number>(32);

  protected readonly initials = computed<string>(() => {
    const u = this.user();
    return (u.firstName?.[0] ?? '') + (u.lastName?.[0] ?? '');
  });
}
