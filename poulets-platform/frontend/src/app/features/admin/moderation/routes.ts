// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./moderation-queue.component').then((m) => m.ModerationQueueComponent),
    title: 'Modération - Poulets BF',
  },
  {
    path: ':id',
    loadComponent: () =>
      import('./workspace/moderation-workspace.component').then((m) => m.ModerationWorkspaceComponent),
    title: 'Workspace modération - Poulets BF',
  },
] as Routes;
