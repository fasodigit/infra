// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./pwa-info.component').then(m => m.PwaInfoComponent),
    title: 'Mode hors-ligne - Poulets Platform',
  },
] as Routes;
