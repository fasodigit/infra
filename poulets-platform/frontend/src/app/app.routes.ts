import { Routes } from '@angular/router';
import { authGuard } from './core/guards/auth.guard';

export const routes: Routes = [
  // Public: Landing page (root)
  {
    path: '',
    loadComponent: () =>
      import('./features/landing/landing.component').then(m => m.LandingComponent),
    pathMatch: 'full',
    title: 'Poulets BF - Marketplace volailles Burkina Faso',
  },

  // Public: Auth routes (login, register, forgot-password)
  {
    path: 'auth',
    loadChildren: () => import('./features/auth/routes'),
  },

  // Public: Mobile Money payment (unguarded for SMS deep-links + smoke tests).
  // Registered BEFORE the guarded `/checkout` group so router matches these
  // literal paths first.
  {
    path: 'pay/:txId',
    loadComponent: () =>
      import('./features/payments/mobile-money-form.component').then(m => m.MobileMoneyFormComponent),
    title: 'Paiement Mobile Money - Poulets BF',
  },
  {
    path: 'checkout/pay/:txId',
    loadComponent: () =>
      import('./features/payments/mobile-money-form.component').then(m => m.MobileMoneyFormComponent),
    title: 'Paiement Mobile Money - Poulets BF',
  },

  // Public: PWA / offline info (stub F5 — pas de guard, consultable hors-ligne)
  {
    path: 'pwa-info',
    loadChildren: () => import('./features/pwa/routes'),
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
      // Cart
      {
        path: 'cart',
        loadChildren: () => import('./features/cart/routes'),
      },
      // Notifications (user)
      {
        path: 'notifications',
        loadChildren: () => import('./features/notifications/routes'),
      },
      // Checkout
      {
        path: 'checkout',
        loadChildren: () => import('./features/checkout/routes'),
      },
      // Payments (guarded — escrow stub F7)
      {
        path: 'payments',
        loadChildren: () => import('./features/payments/routes'),
      },
      // Admin v2 — Claude Design intégré (Phase 3 / 2026-04-30)
      // L'ancien routes.ts (poulets-admin legacy : monitoring, kpis, regions,
      // organizations, workflows, impression) reste sur disque mais n'est plus
      // wiré. Migration ultérieure si besoin.
      {
        path: 'admin',
        loadChildren: () => import('./features/admin/routes-v2'),
      },
    ],
  },

  // 404 (explicit)
  {
    path: '404',
    loadComponent: () =>
      import('./shared/components/error-page/error-page.component').then(m => m.ErrorPageComponent),
    title: 'Page introuvable - Poulets BF',
  },

  // Wildcard → 404
  {
    path: '**',
    loadComponent: () =>
      import('./shared/components/error-page/error-page.component').then(m => m.ErrorPageComponent),
    title: 'Page introuvable - Poulets BF',
  },
];
