// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: ':type/:id',
    loadComponent: () =>
      import('./components/certificate-print.component').then((m) => m.CertificatePrintComponent),
    title: 'Impression - Poulets BF',
  },
] as Routes;
