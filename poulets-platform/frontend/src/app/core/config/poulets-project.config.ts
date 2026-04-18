// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ProjectConfig } from './project-config.token';

export const POULETS_PROJECT_CONFIG: ProjectConfig = {
  projectId: 'poulets',
  appName: 'Poulets BF',
  appShortName: 'Poulets',
  version: '1.1.0',
  brandColor: '#2E7D32',
  accentColor: '#FF8F00',
  logoText: 'Poulets BF',
  logoAsset: 'assets/img/logo-poulets-bf.svg',
  kratosIssuer: 'Poulets BF',
  navItems: [
    { label: 'menu.dashboard',    icon: 'dashboard',      route: '/dashboard' },
    { label: 'menu.marketplace',  icon: 'storefront',     route: '/marketplace/annonces' },
    { label: 'menu.orders',       icon: 'receipt_long',   route: '/orders' },
    { label: 'menu.messaging',    icon: 'chat',           route: '/messaging' },
    { label: 'menu.profile',      icon: 'person',         route: '/profile' },
    { label: 'menu.admin',        icon: 'admin_panel_settings', route: '/admin/monitoring', rolesAllowed: ['ADMIN'] },
  ],
  availableRoles: [
    { value: 'ELEVEUR',    label: 'Éleveur',    icon: 'agriculture',   description: 'Vend des poulets, publie des annonces, gère ses lots et certifications.' },
    { value: 'CLIENT',     label: 'Client',     icon: 'shopping_cart', description: 'Achète des poulets, suit ses commandes, évalue les éleveurs.' },
    { value: 'PRODUCTEUR', label: 'Producteur', icon: 'factory',       description: 'Produit aliments et poussins vendus aux éleveurs.' },
    { value: 'ADMIN',      label: 'Administrateur', icon: 'admin_panel_settings', description: 'Supervise la plateforme, les utilisateurs et les transactions.' },
  ],
};
