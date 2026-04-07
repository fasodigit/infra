import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./map-view.component').then(m => m.MapViewComponent),
    title: 'Carte - Poulets Platform',
  },
] as Routes;
