import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./lots-list.component').then(m => m.LotsListComponent),
    title: 'Mes Lots - Poulets Platform',
  },
  {
    path: ':lotId',
    loadComponent: () =>
      import('./lot-detail.component').then(m => m.LotDetailComponent),
    title: 'Detail Lot - Poulets Platform',
  },
  {
    path: ':lotId/add-weight',
    loadComponent: () =>
      import('./add-weight.component').then(m => m.AddWeightComponent),
    title: 'Ajouter Pesee - Poulets Platform',
  },
] as Routes;
