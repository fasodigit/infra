// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * PgAdminProbe — accès direct Postgres `auth_ms` (V1-V16) pour valider
 * les invariants admin-UI hors REST :
 *   - audit_log immutable (M22) après opérations critiques
 *   - account_capability_grants (M17) post-grant role
 *   - login_history (M14) score + decision
 *   - recovery_codes used_at (M11)
 *   - account_recovery_requests status (M20/M21)
 *   - admin_settings_history (M23)
 *
 * Pattern miroir de `terroir/pg-probe.ts` — `pg` est en
 * optionalDependencies, donc fallback `unavailable=true` si absent ou
 * connexion KO. Specs doivent appeler `isReachable()` en `beforeAll`.
 */

type PgClientLike = {
  connect: () => Promise<void>;
  end: () => Promise<void>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  query: (sql: string, params?: unknown[]) => Promise<{ rows: any[] }>;
};

export interface PgProbeResult<T> {
  unavailable?: boolean;
  reason?: string;
  rows?: T[];
}

export class PgAdminProbe {
  private readonly dsn: string;

  constructor(dsn?: string) {
    this.dsn =
      dsn ??
      process.env.FASO_ADMIN_PG_URL ??
      process.env.AUTH_MS_PG_URL ??
      'postgresql://auth_ms:auth_ms_dev@localhost:5432/auth_ms';
  }

  private async client(): Promise<PgClientLike | null> {
    try {
      const pgPath = 'pg';
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const pgModule: any = await import(/* @vite-ignore */ pgPath);
      const Ctor = pgModule.Client ?? pgModule.default?.Client;
      if (!Ctor) return null;
      const c = new Ctor({ connectionString: this.dsn });
      await c.connect();
      return c as PgClientLike;
    } catch {
      return null;
    }
  }

  async isReachable(): Promise<boolean> {
    const c = await this.client();
    if (!c) return false;
    try {
      await c.query('SELECT 1');
      return true;
    } catch {
      return false;
    } finally {
      await c.end().catch(() => undefined);
    }
  }

  async runOne(
    sql: string,
    params: unknown[] = [],
  ): Promise<PgProbeResult<Record<string, unknown>>> {
    const c = await this.client();
    if (!c) return { unavailable: true, reason: 'pg-driver-or-conn-down' };
    try {
      const res = await c.query(sql, params);
      return { rows: res.rows };
    } catch (e) {
      return {
        unavailable: true,
        reason: `query-error: ${e instanceof Error ? e.message : 'unknown'}`,
      };
    } finally {
      await c.end().catch(() => undefined);
    }
  }

  /** Récupère les N derniers événements audit pour un actor (M22). */
  async tailAudit(actorEmail: string, limit = 5): Promise<PgProbeResult<Record<string, unknown>>> {
    return this.runOne(
      `SELECT id, action, actor_email, target_id, created_at, metadata
         FROM audit_log
        WHERE actor_email = $1
        ORDER BY created_at DESC
        LIMIT $2`,
      [actorEmail, limit],
    );
  }

  /** Compte capacités d'un user (M17). */
  async countCapabilities(userId: string): Promise<PgProbeResult<{ count: number }>> {
    return this.runOne(
      `SELECT COUNT(*)::int AS count
         FROM account_capability_grants
        WHERE grantee_id = $1
          AND revoked_at IS NULL`,
      [userId],
    );
  }

  /** Récupère l'état d'un setting (M23). */
  async getSettingRow(key: string): Promise<PgProbeResult<Record<string, unknown>>> {
    return this.runOne(
      `SELECT key, value, version, updated_at, updated_by
         FROM admin_settings
        WHERE key = $1`,
      [key],
    );
  }

  async getSettingHistory(key: string, limit = 5): Promise<PgProbeResult<Record<string, unknown>>> {
    return this.runOne(
      `SELECT key, value, version, changed_at, changed_by, reason
         FROM admin_settings_history
        WHERE key = $1
        ORDER BY changed_at DESC
        LIMIT $2`,
      [key, limit],
    );
  }

  /** login_history pour M14 risk scoring. */
  async tailLoginHistory(email: string, limit = 5): Promise<PgProbeResult<Record<string, unknown>>> {
    return this.runOne(
      `SELECT id, email, ip, user_agent, score, decision, country, created_at
         FROM login_history
        WHERE email = $1
        ORDER BY created_at DESC
        LIMIT $2`,
      [email, limit],
    );
  }

  /** Recovery codes pour M11. */
  async getRecoveryCodesForUser(userId: string): Promise<PgProbeResult<Record<string, unknown>>> {
    return this.runOne(
      `SELECT id, user_id, used_at, created_at
         FROM recovery_codes
        WHERE user_id = $1`,
      [userId],
    );
  }

  /** Est-ce que la table existe (V<N> appliquée) ? */
  async tableExists(table: string): Promise<boolean> {
    const r = await this.runOne(
      `SELECT 1 FROM information_schema.tables WHERE table_name = $1`,
      [table],
    );
    return !r.unavailable && (r.rows?.length ?? 0) > 0;
  }
}
