import { Routes } from '@angular/router';
import { authGuard } from './core/guards/auth.guard';

export const routes: Routes = [
  // Public: Auth routes (login, register, forgot-password)
  {
    path: 'auth',
    loadChildren: () => import('./features/auth/routes'),
  },

  // Protected: Layout wrapper with sidebar + toolbar
  {
    path: '',
    loadComponent: () =>
      import('./layout/layout.component').then(m => m.LayoutComponent),
    canActivate: [authGuard],
    children: [
      // Dashboard (with role-based redirect)
      {
        path: 'dashboard',
        loadChildren: () => import('./features/dashboard/routes'),
      },
      // Marketplace
      {
        path: 'marketplace',
        loadChildren: () => import('./features/marketplace/routes'),
      },
      // Calendar
      {
        path: 'calendar',
        loadChildren: () => import('./features/calendar/routes'),
      },
      // Growth tracking
      {
        path: 'growth',
        loadChildren: () => import('./features/growth/routes'),
      },
      // Orders
      {
        path: 'orders',
        loadChildren: () => import('./features/orders/routes'),
      },
      // Contracts
      {
        path: 'contracts',
        loadChildren: () => import('./features/contracts/routes'),
      },
      // Messaging
      {
        path: 'messaging',
        loadChildren: () => import('./features/messaging/routes'),
      },
      // Veterinary
      {
        path: 'veterinary',
        loadChildren: () => import('./features/veterinary/routes'),
      },
      // Halal certification
      {
        path: 'halal',
        loadChildren: () => import('./features/halal/routes'),
      },
      // Delivery
      {
        path: 'delivery',
        loadChildren: () => import('./features/delivery/routes'),
      },
      // Map
      {
        path: 'map',
        loadChildren: () => import('./features/map/routes'),
      },
      // Reputation
      {
        path: 'reputation',
        loadChildren: () => import('./features/reputation/routes'),
      },
      // Profile
      {
        path: 'profile',
        loadChildren: () => import('./features/profile/routes'),
      },
      // Default redirect to dashboard
      {
        path: '',
        redirectTo: 'dashboard',
        pathMatch: 'full',
      },
    ],
  },

  // Wildcard redirect
  {
    path: '**',
    redirectTo: 'auth/login',
  },
];
