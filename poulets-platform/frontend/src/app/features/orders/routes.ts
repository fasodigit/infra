import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./orders-list.component').then(m => m.OrdersListComponent),
    title: 'Commandes - Poulets Platform',
  },
  {
    path: 'new',
    loadComponent: () =>
      import('./create-order.component').then(m => m.CreateOrderComponent),
    title: 'Nouvelle commande - Poulets Platform',
  },
  {
    path: ':id',
    loadComponent: () =>
      import('./order-detail.component').then(m => m.OrderDetailComponent),
    title: 'Detail commande - Poulets Platform',
  },
  {
    path: ':id/tracking',
    loadComponent: () =>
      import('./components/order-tracking.component').then(m => m.OrderTrackingComponent),
    title: 'Suivi commande - Poulets BF',
  },
  {
    path: ':id/tracking/legacy',
    loadComponent: () =>
      import('./order-tracking.component').then(m => m.OrderTrackingComponent),
    title: 'Suivi commande (legacy) - Poulets Platform',
  },
] as Routes;
