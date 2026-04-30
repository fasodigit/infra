// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Types partagés API TERROIR mobile-bff.
 * P1 : à compléter avec OpenAPI spec générée par terroir-mobile-bff (:8833).
 */

export type UUID = string;

export interface ApiError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

export interface ApiEnvelope<T> {
  data: T;
  error?: ApiError;
  trace_id?: string;
}

export interface AuthSession {
  jwt: string;
  refresh_token?: string;
  expires_at: number; // epoch seconds
  agent_id: UUID;
  tenant_id: UUID;
}

export interface SyncStatus {
  last_sync_at?: number;
  pending_uploads: number;
  pending_downloads: number;
  conflicts: number;
}

export interface HealthResponse {
  status: 'up' | 'degraded' | 'down';
  version: string;
  bff_reachable: boolean;
}
