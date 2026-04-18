// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

/**
 * Types Kratos self-service flow (login / registration / settings / recovery / verification).
 * Adapté de Etat-civil/frontend/actor-ui/src/app/core/auth/auth.service.ts.
 */
export interface KratosUiNodeAttributes {
  name: string;
  type?: string;
  value?: string | number | boolean | Record<string, unknown>;
  required?: boolean;
  disabled?: boolean;
  node_type?: string;
  onclick?: string;
  pattern?: string;
  label?: { id: number; text: string };
}

export interface KratosUiMessage {
  id: number;
  text: string;
  type: 'info' | 'success' | 'error';
  context?: Record<string, unknown>;
}

export interface KratosUiNode {
  type: 'input' | 'img' | 'a' | 'text' | 'script';
  group: 'default' | 'password' | 'webauthn' | 'totp' | 'lookup_secret' | 'profile' | 'code' | 'oidc' | 'link';
  attributes: KratosUiNodeAttributes;
  messages: KratosUiMessage[];
  meta?: { label?: { id: number; text: string } };
}

export interface KratosUi {
  action: string;
  method: 'POST' | 'GET';
  nodes: KratosUiNode[];
  messages?: KratosUiMessage[];
}

export interface KratosFlow {
  id: string;
  type: 'browser' | 'api';
  expires_at: string;
  issued_at: string;
  request_url: string;
  ui: KratosUi;
  return_to?: string;
}

export interface KratosSettingsFlow extends KratosFlow {
  identity: {
    id: string;
    traits: Record<string, unknown>;
    verifiable_addresses?: Array<{ value: string; verified: boolean; via: string }>;
    credentials?: Record<string, { type: string; identifiers: string[]; config?: Record<string, unknown> }>;
  };
  state: 'show_form' | 'success';
}

export interface KratosSession {
  id: string;
  active: boolean;
  expires_at: string;
  authenticated_at: string;
  authenticator_assurance_level: 'aal0' | 'aal1' | 'aal2';
  identity: {
    id: string;
    traits: Record<string, unknown>;
    verifiable_addresses?: Array<{ value: string; verified: boolean }>;
  };
  devices?: Array<{
    id: string;
    ip_address: string;
    user_agent: string;
    location?: string;
  }>;
}

export interface PasskeyDevice {
  id: string;
  name: string;
  addedAt: string;
  lastUsedAt?: string;
  kind: 'platform' | 'cross-platform' | 'unknown';
}

export interface TOTPConfig {
  configured: boolean;
  configuredAt?: string;
  issuer?: string;
}

export interface BackupCodesConfig {
  generated: boolean;
  remaining: number;
  generatedAt?: string;
}

export interface MfaStatus {
  email: { verified: boolean; address: string };
  passkey: { configured: boolean; devices: PasskeyDevice[] };
  totp: TOTPConfig;
  backupCodes: BackupCodesConfig;
  phone: { configured: boolean; number?: string };
  completed: boolean;
}
