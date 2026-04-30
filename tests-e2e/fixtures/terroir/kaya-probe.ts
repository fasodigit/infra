// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * KayaProbe — direct KAYA probe for setting/getting/deleting flags used
 * by `terroir-mobile-bff` to enforce JWT revocation
 * (`auth:agent:revoked:{userId}=1` checked at each `/m/sync/batch`).
 *
 * KAYA exposes a Redis-compatible RESP3 surface — we use the `redis`
 * npm package dynamically (peer dep). When the lib is absent or KAYA is
 * unreachable, every method returns `unavailable=true` so specs can
 * `test.skip` cleanly rather than crash.
 *
 * Default URL : `redis://localhost:6380` (KAYA primary in port-policy.yaml,
 * §8). Override via `KAYA_URL` or `REDIS_URL` env.
 *
 * Cf. CLAUDE.md §3 (sovereignty) — never call this DragonflyDB or Redis.
 */

import {} from 'node:process';

interface RedisLike {
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  set: (k: string, v: string) => Promise<unknown>;
  get: (k: string) => Promise<string | null>;
  del: (k: string) => Promise<unknown>;
  ping: () => Promise<unknown>;
}

export interface KayaProbeResult<T> {
  unavailable?: boolean;
  reason?: string;
  value?: T;
}

export class KayaProbe {
  private readonly url: string;

  constructor(url?: string) {
    this.url =
      url ??
      process.env.KAYA_URL ??
      process.env.REDIS_URL ??
      'redis://localhost:6380';
  }

  private async client(): Promise<RedisLike | null> {
    try {
      // Dynamic import (runtime-only path so tsc doesn't require types).
      const redisPath = 'redis';
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const mod: any = await import(/* @vite-ignore */ redisPath);
      const factory = mod.createClient ?? mod.default?.createClient;
      if (!factory) return null;
      const c: RedisLike = factory({ url: this.url });
      await c.connect();
      return c;
    } catch {
      return null;
    }
  }

  async setFlag(
    key: string,
    value = '1',
  ): Promise<KayaProbeResult<string>> {
    const c = await this.client();
    if (!c) return { unavailable: true, reason: 'kaya-down-or-no-driver' };
    try {
      await c.set(key, value);
      return { value };
    } catch (e) {
      return {
        unavailable: true,
        reason: `set-error: ${e instanceof Error ? e.message : 'unknown'}`,
      };
    } finally {
      await c.disconnect().catch(() => undefined);
    }
  }

  async getFlag(key: string): Promise<KayaProbeResult<string | null>> {
    const c = await this.client();
    if (!c) return { unavailable: true, reason: 'kaya-down-or-no-driver' };
    try {
      const v = await c.get(key);
      return { value: v };
    } catch (e) {
      return {
        unavailable: true,
        reason: `get-error: ${e instanceof Error ? e.message : 'unknown'}`,
      };
    } finally {
      await c.disconnect().catch(() => undefined);
    }
  }

  async delFlag(key: string): Promise<KayaProbeResult<true>> {
    const c = await this.client();
    if (!c) return { unavailable: true, reason: 'kaya-down-or-no-driver' };
    try {
      await c.del(key);
      return { value: true };
    } catch (e) {
      return {
        unavailable: true,
        reason: `del-error: ${e instanceof Error ? e.message : 'unknown'}`,
      };
    } finally {
      await c.disconnect().catch(() => undefined);
    }
  }

  async isReachable(): Promise<boolean> {
    const c = await this.client();
    if (!c) return false;
    try {
      await c.ping();
      return true;
    } catch {
      return false;
    } finally {
      await c.disconnect().catch(() => undefined);
    }
  }
}
