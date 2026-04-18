// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components/notifications-inbox.component').then((m) => m.NotificationsInboxComponent),
    title: 'Notifications - Poulets BF',
  },
] as Routes;
