import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./deliveries-list.component').then(m => m.DeliveriesListComponent),
    title: 'Livraisons - Poulets Platform',
  },
  {
    path: ':id',
    loadComponent: () =>
      import('./delivery-detail.component').then(m => m.DeliveryDetailComponent),
    title: 'Detail livraison - Poulets Platform',
  },
] as Routes;
