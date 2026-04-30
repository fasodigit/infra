// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * Wrapper d'audit côté BFF.
 *
 * Le BFF lui-même ne touche jamais la table `audit_log` — il appelle
 * l'endpoint interne `auth-ms` `POST /internal/admin/audit-log` (à créer
 * côté Stream A). auth-ms persiste l'événement en base + publie sur
 * Redpanda `admin.audit.event` pour les consumers downstream.
 *
 * Les appels sont fire-and-forget côté BFF (avec log d'erreur) — la
 * persistance fiable est assurée par auth-ms (transactional outbox).
 */

import { generateRequestId } from '@/lib/admin-otp';

const AUTH_MS_URL = process.env.AUTH_MS_URL ?? 'http://localhost:8801';

export interface AuditActor {
  userId: string;
  email?: string;
  role?: string;
  ip?: string;
  userAgent?: string;
}

export interface AuditTarget {
  type: string;
  id?: string;
  label?: string;
}

export interface AuditEnvelope {
  action: string;
  actor: AuditActor;
  target?: AuditTarget;
  metadata?: Record<string, unknown>;
  traceId?: string;
  oldValue?: unknown;
  newValue?: unknown;
  critical?: boolean;
}

/**
 * Émet un évènement d'audit. Renvoie `true` si la requête upstream a
 * abouti (2xx), `false` sinon. **Fail-open** côté BFF : on n'aborte
 * jamais la route admin parce que l'audit a échoué — mais on log.
 */
export async function auditLog(
  envelope: AuditEnvelope,
  options: { authToken?: string; idempotencyKey?: string } = {},
): Promise<boolean> {
  const idempotencyKey = options.idempotencyKey ?? generateRequestId();
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    'Idempotency-Key': idempotencyKey,
  };
  if (options.authToken) headers['Authorization'] = `Bearer ${options.authToken}`;
  if (envelope.traceId) headers['traceparent'] = envelope.traceId;

  try {
    const res = await fetch(`${AUTH_MS_URL}/internal/admin/audit-log`, {
      method: 'POST',
      headers,
      body: JSON.stringify({
        action: envelope.action,
        actor: envelope.actor,
        target: envelope.target ?? null,
        metadata: envelope.metadata ?? {},
        oldValue: envelope.oldValue ?? null,
        newValue: envelope.newValue ?? null,
        critical: envelope.critical === true,
        traceId: envelope.traceId ?? null,
        emittedAt: new Date().toISOString(),
      }),
      cache: 'no-store',
    });
    if (!res.ok) {
      console.error(
        '[admin-audit] upstream returned non-2xx',
        res.status,
        envelope.action,
      );
      return false;
    }
    return true;
  } catch (err) {
    console.error('[admin-audit] fetch failed', envelope.action, err);
    return false;
  }
}
