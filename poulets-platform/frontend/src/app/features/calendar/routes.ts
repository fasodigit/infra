import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components/calendar-view.component').then(m => m.CalendarViewComponent),
    title: 'Calendrier - Poulets Platform',
  },
  {
    path: 'planning',
    loadComponent: () =>
      import('./components/planning.component').then(m => m.PlanningComponent),
    title: 'Planning Offre/Demande - Poulets Platform',
  },
] as Routes;
