// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component } from '@angular/core';
import { AdminShellComponent } from '@shared/components/admin-shell/admin-shell.component';

@Component({
  selector: 'app-admin-layout',
  standalone: true,
  imports: [AdminShellComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `<app-admin-shell />`,
})
export class AdminLayoutComponent {}
