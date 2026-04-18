// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { UserRole, AdminLevel } from '@core/config/project-config.token';

export type ServiceStatus = 'UP' | 'DEGRADED' | 'DOWN';
export type AlertSeverity = 'info' | 'warning' | 'critical';
export type AuditResult = 'SUCCESS' | 'FAILURE' | 'DENIED';

export interface ServiceHealth {
  name: string;
  category: 'gateway' | 'cache' | 'auth' | 'db' | 'broker' | 'app';
  status: ServiceStatus;
  latencyP99Ms: number;
  requestsPerSec: number;
  errorRate: number;
  uptime: string;
  lastCheck: string;
}

export interface SystemAlert {
  id: string;
  type: string;
  severity: AlertSeverity;
  service: string;
  message: string;
  createdAt: string;
  acknowledged: boolean;
  acknowledgedBy?: string;
}

export interface AuditLog {
  id: string;
  timestamp: string;
  action: string;
  user: string;
  userRole?: UserRole;
  resource: string;
  resourceId?: string;
  ipAddress: string;
  userAgent?: string;
  result: AuditResult;
  detail?: string;
}

export interface PlatformUser {
  id: string;
  email: string;
  firstName: string;
  lastName: string;
  displayName: string;
  role: UserRole;
  adminLevel?: AdminLevel;
  phone?: string;
  region?: string;
  isActive: boolean;
  mfaConfigured: boolean;
  mfaStatus: {
    email: boolean;
    passkey: boolean;
    totp: boolean;
    backupCodes: boolean;
    phone: boolean;
  };
  createdAt: string;
  lastLoginAt?: string;
  /** Rôle-spécifique : ELEVEUR/PRODUCTEUR/CLIENT/ADMIN */
  roleMeta?: Record<string, unknown>;
}

export interface ActiveUsersStat {
  total: number;
  byRole: Record<UserRole, number>;
}

export interface FeatureFlag {
  key: string;
  label: string;
  description?: string;
  enabled: boolean;
  rolloutPercent?: number;
  updatedAt?: string;
}
