// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * fetch wrapper TERROIR mobile-bff.
 *
 * Cible : ARMAGEDDON :8080 → cluster terroir-mobile-bff (:8833).
 * En dev Android emulator : 10.0.2.2 = host loopback.
 * En dev iOS simulator : localhost.
 * En prod : URL Vault `faso/terroir/mobile/api-base-url`.
 *
 * Tracing : injection W3C `traceparent` aléatoire — corrélé avec spans
 * Tempo backend (cf. INFRA/observability/grafana/podman-compose.observability.yml).
 */
import Constants from 'expo-constants';

import { loadJwt } from '../auth/jwt-storage';
import type { ApiEnvelope, ApiError } from './types';

const DEFAULT_BASE_URL = 'http://10.0.2.2:8080/api/terroir/mobile-bff';

export interface RequestOptions {
  method?: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  body?: unknown;
  headers?: Record<string, string>;
  timeoutMs?: number;
}

export class ApiClientError extends Error {
  constructor(
    public readonly status: number,
    public readonly apiError?: ApiError,
  ) {
    super(apiError?.message ?? `HTTP ${status}`);
    this.name = 'ApiClientError';
  }
}

function getBaseUrl(): string {
  const extra = (Constants.expoConfig?.extra ?? {}) as { apiBaseUrl?: string };
  return extra.apiBaseUrl ?? DEFAULT_BASE_URL;
}

/**
 * Génère un traceparent W3C v00 (32 hex trace-id + 16 hex span-id, sampled).
 * https://www.w3.org/TR/trace-context/
 */
function generateTraceparent(): string {
  const hex = (bytes: number) =>
    Array.from({ length: bytes }, () =>
      Math.floor(Math.random() * 256)
        .toString(16)
        .padStart(2, '0'),
    ).join('');
  const traceId = hex(16);
  const spanId = hex(8);
  return `00-${traceId}-${spanId}-01`;
}

export async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const { method = 'GET', body, headers = {}, timeoutMs = 15_000 } = options;
  const baseUrl = getBaseUrl();
  const url = path.startsWith('http') ? path : `${baseUrl}${path}`;

  const jwt = await loadJwt();
  const finalHeaders: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: 'application/json',
    traceparent: generateTraceparent(),
    'User-Agent': 'TerroirMobile/0.1.0 (Expo SDK 53)',
    ...headers,
  };
  if (jwt) {
    finalHeaders.Authorization = `Bearer ${jwt}`;
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const response = await fetch(url, {
      method,
      headers: finalHeaders,
      body: body !== undefined ? JSON.stringify(body) : undefined,
      signal: controller.signal,
    });

    const text = await response.text();
    const json: ApiEnvelope<T> | T | undefined = text ? JSON.parse(text) : undefined;

    if (!response.ok) {
      const apiError =
        json && typeof json === 'object' && 'error' in json
          ? (json as ApiEnvelope<T>).error
          : undefined;
      throw new ApiClientError(response.status, apiError);
    }

    if (json && typeof json === 'object' && 'data' in json) {
      return (json as ApiEnvelope<T>).data;
    }
    return json as T;
  } finally {
    clearTimeout(timeout);
  }
}

export const apiClient = {
  get: <T>(path: string, options?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(path, { ...options, method: 'GET' }),
  post: <T>(path: string, body?: unknown, options?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(path, { ...options, method: 'POST', body }),
  put: <T>(path: string, body?: unknown, options?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(path, { ...options, method: 'PUT', body }),
  delete: <T>(path: string, options?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(path, { ...options, method: 'DELETE' }),
};
