// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * Helpers permissions admin.
 *
 * - requireRole(level)        : compare la hiérarchie de rôles
 *                               SUPER-ADMIN > ADMIN > MANAGER.
 * - requireKetoCheck(...)     : appelle Keto :4466 /relation-tuples/check
 *                               et renvoie `true`/`false`.
 *
 * Ces helpers sont consommés par `lib/admin-auth.ts` mais peuvent aussi
 * être appelés directement par des routes pour des checks fins (par ex.
 * `update_settings` qui exige SUPER-ADMIN strict).
 */

import type { AdminLevel } from '@/lib/schemas/admin';

const KETO_READ_URL = process.env.KETO_READ_URL ?? 'http://localhost:4466';

const LEVEL_RANK: Record<AdminLevel, number> = {
  'SUPER-ADMIN': 3,
  ADMIN: 2,
  MANAGER: 1,
};

/**
 * Vérifie qu'un rôle effectif satisfait au moins le niveau requis.
 * `actual` peut être `undefined` (utilisateur non-admin) → renvoie false.
 */
export function requireRole(
  actual: AdminLevel | string | undefined | null,
  required: AdminLevel,
): boolean {
  if (!actual) return false;
  const normalized = normalizeLevel(actual);
  if (!normalized) return false;
  return LEVEL_RANK[normalized] >= LEVEL_RANK[required];
}

/**
 * Normalise un rôle (case-insensitive, accepte les variantes Kratos
 * `super-admin` / `admin` / `manager`) vers le canonical `AdminLevel`.
 */
export function normalizeLevel(input: string): AdminLevel | undefined {
  const v = input.toLowerCase().replace(/_/g, '-');
  if (v === 'super-admin' || v === 'superadmin') return 'SUPER-ADMIN';
  if (v === 'admin') return 'ADMIN';
  if (v === 'manager') return 'MANAGER';
  return undefined;
}

export interface KetoCheckParams {
  namespace: string;
  object: string;
  relation: string;
  subjectId: string;
  traceparent?: string;
}

/**
 * Appelle Keto `/relation-tuples/check` pour valider qu'un sujet possède
 * une relation sur un objet. Renvoie `false` en cas d'erreur réseau ou
 * de réponse non-2xx — décision *fail-closed*.
 */
export async function requireKetoCheck(params: KetoCheckParams): Promise<boolean> {
  try {
    const url = new URL(`${KETO_READ_URL}/relation-tuples/check`);
    url.searchParams.set('namespace', params.namespace);
    url.searchParams.set('object', params.object);
    url.searchParams.set('relation', params.relation);
    url.searchParams.set('subject_id', params.subjectId);

    const headers: Record<string, string> = { Accept: 'application/json' };
    if (params.traceparent) headers['traceparent'] = params.traceparent;

    const res = await fetch(url.toString(), {
      method: 'GET',
      headers,
      cache: 'no-store',
    });
    if (!res.ok) return false;
    const json = (await res.json()) as { allowed?: boolean };
    return json?.allowed === true;
  } catch (err) {
    console.error('[admin-permissions] Keto check failed', err);
    return false;
  }
}
