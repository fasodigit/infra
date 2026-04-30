// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// FASO functional matrix — 17 features × ~6 active roles per feature.
// Drives the parameterised tests in tests/18-functional-matrix/.

import type { ActorRole } from './actors';

export type Feature =
  | 'signup'
  | 'profile-view'
  | 'profile-edit'
  | 'marketplace-browse'
  | 'marketplace-post-offer'
  | 'marketplace-post-demand'
  | 'order-create'
  | 'order-accept'
  | 'order-cancel'
  | 'halal-certify'
  | 'vaccine-record'
  | 'pharmacy-stock'
  | 'delivery-accept'
  | 'messaging-send'
  | 'payment-init'
  | 'reputation-view'
  | 'reputation-write'
  | 'admin-dashboard'
  | 'admin-impersonate'
  | 'kyc-upload'
  | 'push-notif-subscribe'
  | 'push-notif-unsubscribe'
  | 'audit-log-read';

export type ExpectedOutcome = 'allow' | 'deny';

export interface MatrixCell {
  feature: Feature;
  role: ActorRole;
  expected: ExpectedOutcome;
  /** Path called via gateway (for direct gateway tests). */
  path?: string;
  /** HTTP method (for direct gateway tests). */
  method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
}

const ALL_ROLES: ActorRole[] = [
  'eleveur', 'client', 'pharmacie', 'veterinaire',
  'aliments', 'transporteur', 'vaccins', 'admin',
];

/**
 * Helper: build matrix cells for a feature given the allowed roles.
 * Roles outside `allowedRoles` get `expected: 'deny'`, hence the matrix
 * exercises BOTH happy paths AND authz-denial paths.
 */
function row(
  feature: Feature,
  allowedRoles: ActorRole[] | '*',
  path?: string,
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' = 'GET',
): MatrixCell[] {
  const allowed: ActorRole[] = allowedRoles === '*' ? ALL_ROLES : allowedRoles;
  return ALL_ROLES.map((role) => ({
    feature,
    role,
    expected: allowed.includes(role) ? 'allow' : 'deny',
    path,
    method,
  }));
}

/**
 * Full matrix : ~22 features × 8 roles = 176 cells.
 * Each spec from tests/18-functional-matrix/ paramétrise sur ces cellules.
 */
export const FEATURE_MATRIX: MatrixCell[] = [
  // Signup — admin gets seeded, others self-register
  ...row('signup',                  ['eleveur','client','pharmacie','veterinaire','aliments','transporteur','vaccins'], '/auth/register', 'POST'),
  // Profile
  ...row('profile-view',            '*',                                                                                '/api/profile',   'GET'),
  ...row('profile-edit',            '*',                                                                                '/api/profile',   'PATCH'),
  // Marketplace
  ...row('marketplace-browse',      '*',                                                                                '/api/annonces',  'GET'),
  ...row('marketplace-post-offer',  ['eleveur','admin'],                                                                '/api/annonces',  'POST'),
  ...row('marketplace-post-demand', ['client','admin'],                                                                 '/api/besoins',   'POST'),
  // Orders
  ...row('order-create',            ['client','admin'],                                                                 '/api/commandes', 'POST'),
  ...row('order-accept',            ['eleveur','admin'],                                                                '/api/commandes', 'PATCH'),
  ...row('order-cancel',            ['client','eleveur','admin'],                                                       '/api/commandes', 'DELETE'),
  // Vétérinaire / Pharmacie / Vaccins
  ...row('halal-certify',           ['veterinaire','admin'],                                                            '/api/halal/certify', 'POST'),
  ...row('vaccine-record',          ['veterinaire','vaccins','admin'],                                                  '/api/vaccines',  'POST'),
  ...row('pharmacy-stock',          ['pharmacie','admin'],                                                              '/api/pharmacy',  'GET'),
  // Transporteur
  ...row('delivery-accept',         ['transporteur','admin'],                                                           '/api/delivery',  'POST'),
  // Messaging
  ...row('messaging-send',          '*',                                                                                '/api/messaging', 'POST'),
  // Payment (clients pay, admins refund)
  ...row('payment-init',            ['client','admin'],                                                                 '/api/payments',  'POST'),
  // Reputation — read all, write only after a transaction
  ...row('reputation-view',         '*',                                                                                '/api/reputation','GET'),
  ...row('reputation-write',        ['eleveur','client','admin'],                                                       '/api/reputation','POST'),
  // Admin
  ...row('admin-dashboard',         ['admin'],                                                                          '/admin/dashboard','GET'),
  ...row('admin-impersonate',       ['admin'],                                                                          '/admin/impersonate','POST'),
  // KYC — all human roles upload (admins are pre-vetted)
  ...row('kyc-upload',              ['eleveur','client','pharmacie','veterinaire','aliments','transporteur','vaccins'], '/api/profile/kyc',  'POST'),
  // Push notif
  ...row('push-notif-subscribe',    '*',                                                                                '/api/profile/notifications-push','POST'),
  ...row('push-notif-unsubscribe',  '*',                                                                                '/api/profile/notifications-push','DELETE'),
  // Audit log read — admin only (Loi 010-2004 access control)
  ...row('audit-log-read',          ['admin'],                                                                          '/admin/audit-log','GET'),
];

/** Filter helper for parametrised describe blocks. */
export function cellsForFeature(feature: Feature): MatrixCell[] {
  return FEATURE_MATRIX.filter((c) => c.feature === feature);
}

export function cellsForRole(role: ActorRole): MatrixCell[] {
  return FEATURE_MATRIX.filter((c) => c.role === role);
}

/** Coverage assertion helper: returns count of allow / deny per feature. */
export function coverageStats(): Array<{ feature: Feature; allow: number; deny: number }> {
  const byFeature = new Map<Feature, { allow: number; deny: number }>();
  for (const c of FEATURE_MATRIX) {
    const e = byFeature.get(c.feature) ?? { allow: 0, deny: 0 };
    if (c.expected === 'allow') e.allow++; else e.deny++;
    byFeature.set(c.feature, e);
  }
  return [...byFeature.entries()].map(([feature, s]) => ({ feature, ...s }));
}
