// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { InjectionToken } from '@angular/core';

/** Identifiant du projet FASO DIGITALISATION (7 plateformes). */
export type ProjectId =
  | 'poulets'
  | 'etat-civil'
  | 'sogesy'
  | 'hospital'
  | 'faso-kalan'
  | 'vouchers'
  | 'e-ticket';

export type UserRole = 'ELEVEUR' | 'CLIENT' | 'PRODUCTEUR' | 'ADMIN';

export type AdminLevel = 'SUPER_ADMIN' | 'ADMIN_MODERATION' | 'ADMIN_SUPPORT';

export interface NavItem {
  readonly label: string;
  readonly icon: string;
  readonly route: string;
  readonly badge?: string;
  readonly rolesAllowed?: UserRole[];
  readonly superAdminOnly?: boolean;
}

export interface RoleOption {
  readonly value: UserRole;
  readonly label: string;
  readonly description: string;
  readonly icon: string;
}

export interface ProjectConfig {
  readonly projectId: ProjectId;
  readonly appName: string;
  readonly appShortName: string;
  readonly version: string;
  readonly brandColor: string;
  readonly accentColor: string;
  readonly logoText: string;
  readonly logoAsset: string;
  readonly kratosIssuer: string;
  readonly navItems: NavItem[];
  readonly availableRoles: RoleOption[];
}

export const PROJECT_CONFIG = new InjectionToken<ProjectConfig>('PROJECT_CONFIG');
