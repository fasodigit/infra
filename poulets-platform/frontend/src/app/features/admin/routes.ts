// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';
import { adminGuard } from '@core/guards/admin.guard';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components/admin-layout.component').then((m) => m.AdminLayoutComponent),
    canActivate: [adminGuard],
    children: [
      { path: '', redirectTo: 'monitoring', pathMatch: 'full' },
      {
        path: 'monitoring',
        loadComponent: () =>
          import('./components/admin-monitoring.component').then((m) => m.AdminMonitoringComponent),
        title: 'Monitoring - Poulets BF',
      },
      {
        path: 'audit',
        loadComponent: () =>
          import('./components/admin-audit.component').then((m) => m.AdminAuditComponent),
        title: "Logs d'audit - Poulets BF",
      },
      {
        path: 'platform-config',
        loadComponent: () =>
          import('./components/admin-platform-config.component').then((m) => m.AdminPlatformConfigComponent),
        title: 'Configuration plateforme - Poulets BF',
      },
      {
        path: 'users',
        loadChildren: () => import('../users/routes'),
      },
      {
        path: 'kpis',
        loadComponent: () =>
          import('./components/admin-kpis.component').then((m) => m.AdminKpisComponent),
        title: 'KPIs - Poulets BF',
      },
      {
        path: 'regions',
        loadComponent: () =>
          import('./components/admin-regions.component').then((m) => m.AdminRegionsComponent),
        title: 'Régions BF - Poulets BF',
      },
      {
        path: 'notifications-config',
        loadComponent: () =>
          import('./components/admin-notifications-config.component').then((m) => m.AdminNotificationsConfigComponent),
        title: 'Configuration notifications - Poulets BF',
      },
      {
        path: 'moderation',
        loadChildren: () => import('./moderation/routes'),
      },
      {
        path: 'organizations',
        loadChildren: () => import('./organizations/routes'),
      },
      {
        path: 'workflows',
        loadChildren: () => import('./workflows/routes'),
      },
      {
        path: 'impression',
        loadChildren: () => import('./impression/routes'),
      },
    ],
  },
] as Routes;
