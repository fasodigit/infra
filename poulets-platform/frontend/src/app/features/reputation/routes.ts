import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./reputation-view.component').then(m => m.ReputationViewComponent),
    title: 'Reputation - Poulets Platform',
  },
  {
    path: 'review/:userId',
    loadComponent: () =>
      import('./leave-review.component').then(m => m.LeaveReviewComponent),
    title: 'Laisser un avis - Poulets Platform',
  },
] as Routes;
