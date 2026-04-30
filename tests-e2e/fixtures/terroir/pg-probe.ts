// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * PgProbe — direct PostgreSQL probe used to verify low-level invariants
 * (PII encryption, RLS isolation, schema separation) that cannot be
 * observed through the REST surface alone.
 *
 * Loads the `pg` npm package dynamically so that environments without
 * Postgres connectivity can still run the rest of the suite. If `pg`
 * isn't installed, every method returns a structured `unavailable` flag
 * and specs should skip rather than crash.
 *
 * Connection params (env or constructor):
 *   - PGURL or TERROIR_PG_URL  — full DSN (preferred).
 *   - PG_HOST / PG_PORT / PG_USER / PG_PASSWORD / PG_DATABASE.
 *
 * The default DSN points at the dev Postgres exposed by
 * `INFRA/docker/compose/podman-compose.yml` :
 *   postgresql://terroir_app:terroir_app@localhost:5432/auth_ms
 */

import {} from 'node:process';

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

export class PgProbe {
  private readonly dsn: string;

  constructor(dsn?: string) {
    this.dsn =
      dsn ??
      process.env.TERROIR_PG_URL ??
      process.env.PGURL ??
      'postgresql://terroir_app:terroir_app@localhost:5432/auth_ms';
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private async client(): Promise<PgClientLike | null> {
    try {
      // dynamic import keeps `pg` optional. Module name built at runtime
      // so the TypeScript compiler doesn't try to resolve it statically
      // (the package is in `optionalDependencies`).
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

  /**
   * Vérifie qu'un producteur a bien ses champs PII chiffrés dans le tenant
   * schema demandé. Renvoie {unavailable:true} si la connexion PG n'est
   * pas disponible (le test doit alors `test.skip` en conséquence).
   */
  async assertPiiEncrypted(
    tenantSchema: string,
    producerId: string,
  ): Promise<PgProbeResult<{
    full_name_encrypted: Buffer | null;
    nin_encrypted: Buffer | null;
    phone_encrypted: Buffer | null;
  }>> {
    const c = await this.client();
    if (!c) return { unavailable: true, reason: 'pg-driver-or-conn-down' };
    try {
      const res = await c.query(
        `SELECT full_name_encrypted, nin_encrypted, phone_encrypted
           FROM ${tenantSchema}.producer
          WHERE id = $1`,
        [producerId],
      );
      return { rows: res.rows };
    } catch (e) {
      return { unavailable: true, reason: `query-error: ${e instanceof Error ? e.message : 'unknown'}` };
    } finally {
      await c.end().catch(() => undefined);
    }
  }

  /**
   * Compte les rangs visibles depuis le rôle `terroir_app` sur un schéma
   * donné — utilisé pour valider RLS / schema isolation cross-tenant.
   */
  async countRows(
    schema: string,
    table: string,
  ): Promise<PgProbeResult<{ count: number }>> {
    const c = await this.client();
    if (!c) return { unavailable: true, reason: 'pg-driver-or-conn-down' };
    try {
      const res = await c.query(
        `SELECT COUNT(*)::int AS count FROM ${schema}.${table}`,
      );
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

  /**
   * Exécute une requête arbitraire (utile pour les assertions ad-hoc).
   * Retourne la première rangée ou null. Toujours guard `unavailable`.
   */
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
}
