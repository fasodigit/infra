// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

export type AdminLevel = 'SUPER-ADMIN' | 'ADMIN' | 'MANAGER';
export type AdminStatus = 'active' | 'suspended';
export type ServiceStatus = 'ok' | 'warn' | 'down';
export type AccentName = 'green' | 'gold' | 'sober';
export type ThemeName = 'light' | 'dark';
export type AdminLang = 'fr' | 'en';

export interface AdminUser {
  id: string;
  firstName: string;
  lastName: string;
  email: string;
  phone?: string;
  department: string;
  role: AdminLevel;
  level: 0 | 1 | 2;
  verified: boolean;
  mfa: { passkey: boolean; totp: boolean; backupCodes: number };
  createdAt: string;
  lastActive: string;
  status: AdminStatus;
  failedLogins: number;
  devices: number;
  avatar: string;
}

export interface AdminSession {
  id: string;
  user: string;
  token: string;
  created: string;
  lastActive: string;
  ip: string;
  city: string;
  device: string;
  current?: boolean;
}

export interface TrustedDevice {
  id: string;
  user: string;
  fp: string;
  type: string;
  ua: string;
  ip: string;
  city: string;
  created: string;
  lastUsed: string;
  trustedUntil: string;
}

export type AuditAction =
  | 'USER_CREATED' | 'USER_SUSPENDED' | 'USER_REACTIVATED'
  | 'ROLE_GRANTED' | 'ROLE_REVOKED'
  | 'OTP_ISSUED' | 'OTP_VERIFIED' | 'OTP_FAILED'
  | 'MFA_ENROLLED' | 'MFA_REMOVED'
  | 'DEVICE_TRUSTED' | 'DEVICE_REVOKED'
  | 'SESSION_REVOKED' | 'PASSWORD_RESET'
  | 'RECOVERY_CODES_GENERATED' | 'RECOVERY_CODE_USED' | 'RECOVERY_CODE_INVALID'
  | 'BREAK_GLASS_ACTIVATED' | 'BREAK_GLASS_EXPIRED'
  | 'SETTINGS_UPDATED' | 'SETTINGS_REVERTED'
  | 'SUPER_ADMIN_PROTECTION_TRIGGERED'
  | 'CAPABILITY_GRANTED' | 'CAPABILITY_REVOKED' | 'CAPABILITY_SET_DUPLICATE_OVERRIDE'
  | 'ACCOUNT_RECOVERY_SELF_INITIATED'
  | 'ACCOUNT_RECOVERY_ADMIN_INITIATED'
  | 'ACCOUNT_RECOVERY_COMPLETED'
  // Phase 4.b.4 — Magic-link channel-binding
  | 'MAGIC_LINK_ISSUED'
  | 'MAGIC_LINK_VERIFIED'
  | 'MAGIC_LINK_REPLAYED'
  | 'ONBOARD_COMPLETED'
  // Phase 4.b.3 — Crypto upgrade Argon2id + HMAC pepper
  | 'HASH_REHASHED_ON_LOGIN'
  // Phase 4.b.6 — Risk-based scoring MVP
  | 'LOGIN_RISK_ASSESSED'
  | 'LOGIN_BLOCKED_HIGH_RISK'
  | 'LOGIN_STEP_UP_REQUIRED'
  // Phase 4.b.7 — Step-up auth pour opérations sensibles
  | 'STEP_UP_REQUESTED'
  | 'STEP_UP_VERIFIED'
  | 'STEP_UP_FAILED';

export interface AuditEntry {
  id: string;
  actor: string;
  action: AuditAction;
  target: string;
  desc: string;
  time: string;
  date: string;
  traceId: string;
  ip: string;
  oldVal?: unknown;
  newVal?: unknown;
  critical?: boolean;
}

export interface ServiceHealth {
  name: string;
  port: string;
  status: ServiceStatus;
  meta: string;
}

export interface ChartPoint {
  d: string;
  otp: number;
  sessions: number;
}

export type SettingValueType = 'bool' | 'int' | 'text' | 'enum' | 'enum[]' | 'record';
export type SettingCategory = 'otp' | 'device_trust' | 'session' | 'mfa' | 'grant' | 'break_glass' | 'audit';

export interface AdminSetting<T = unknown> {
  key: string;
  category: SettingCategory;
  value: T;
  valueType: SettingValueType;
  defaultValue: T;
  minValue?: T;
  maxValue?: T;
  requiredRoleToEdit: AdminLevel;
  version: number;
  updatedAt: string;
  updatedBy: string;
  descriptionI18nKey: string;
}

export interface SettingHistoryEntry {
  v: number;
  when: string;
  who: string;
  oldV: unknown;
  newV: unknown;
  motif?: string;
  trace: string;
}

export interface RoleGrantRequest {
  userId: string;
  targetRole: AdminLevel;
  justification: string;
  otpCode: string;
  scope?: 'TOUS' | 'DIRECTION';
  tenantId?: string;
  capabilities?: readonly string[];
  forceDuplicate?: boolean;
}

export type CapabilityDomain =
  | 'users'
  | 'sessions'
  | 'devices'
  | 'mfa'
  | 'audit'
  | 'settings'
  | 'break_glass'
  | 'recovery'
  | 'roles';

export interface Capability {
  /** Clé canonique, ex: `users:invite`, `roles:grant_admin`. */
  readonly key: string;
  readonly domain: CapabilityDomain;
  /** Rôles maximaux pouvant porter la capacité (filtre côté UI). */
  readonly availableForRoles: readonly AdminLevel[];
  readonly i18nLabelKey: string;
}

export interface CapabilityUniquenessCheck {
  readonly duplicate: boolean;
  readonly matchedUserId?: string;
  readonly matchedUserEmail?: string;
}

export interface BreakGlassRequest {
  capability: 'db' | 'grant' | 'settings';
  justification: string;
  otpCode: string;
}

export interface AdminAlert {
  id: string;
  severity: 'info' | 'warning' | 'critical';
  title: string;
  description: string;
  affectedService: string;
  createdAt: string;
  acknowledged: boolean;
}

export interface AdminDashboardKpis {
  activeUsers7d: number;
  otpSent24h: number;
  activeSessions: number;
  unacknowledgedAlerts: number;
  chart: ChartPoint[];
  servicesHealth: ServiceHealth[];
}
