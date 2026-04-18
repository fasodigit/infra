// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components/workflow-monitoring.component').then((m) => m.WorkflowMonitoringComponent),
    title: 'Workflows Temporal - Poulets BF',
  },
  {
    path: ':id',
    loadComponent: () =>
      import('./components/workflow-detail.component').then((m) => m.WorkflowDetailComponent),
    title: 'Détail workflow - Poulets BF',
  },
] as Routes;
