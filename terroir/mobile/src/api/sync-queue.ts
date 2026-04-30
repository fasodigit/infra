// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Queue offline TERROIR — table SQLite `sync_queue`.
 *
 * Mode offline-first : chaque mutation (create producer, polygon update,
 * agronomy note, …) est enfilée localement. Un worker périodique (60s,
 * démarré au login) groupe par batch ≤ `SYNC_BATCH_MAX_ITEMS = 100`,
 * appelle `postSyncBatch()`, met à jour le statut.
 *
 * Schéma :
 * ```sql
 * CREATE TABLE IF NOT EXISTS sync_queue (
 *   id            TEXT PRIMARY KEY,           -- UUID v4 client
 *   type          TEXT NOT NULL,              -- ex 'producer-create', 'parcel-polygon-update'
 *   payload_json  TEXT NOT NULL,              -- JSON serialisé SyncItem
 *   created_at    INTEGER NOT NULL DEFAULT (strftime('%s','now')),
 *   retry_count   INTEGER NOT NULL DEFAULT 0,
 *   status        TEXT NOT NULL DEFAULT 'pending',  -- pending | sending | ok | error
 *   last_error    TEXT
 * );
 * CREATE INDEX IF NOT EXISTS idx_sync_queue_status ON sync_queue(status, created_at);
 * ```
 *
 * Stratégie retry : exponential backoff 5s/30s/2min/10min, abandon après 5 échecs.
 * P3+ : DLQ visualisée dans SyncStatusScreen.
 */
import * as SQLite from 'expo-sqlite';

import {
  postSyncBatch,
  type SyncBatchRequest,
  type SyncBatchResponse,
  type SyncItem,
} from './mobile-bff-client';

const DB_NAME = 'terroir-sync.db';
const BATCH_MAX = 100;
const WORKER_INTERVAL_MS = 60_000;
const MAX_RETRIES = 5;

export type SyncQueueStatus = 'pending' | 'sending' | 'ok' | 'error';

export interface SyncQueueRow {
  id: string;
  type: string;
  payload_json: string;
  created_at: number;
  retry_count: number;
  status: SyncQueueStatus;
  last_error?: string;
}

let db: SQLite.SQLiteDatabase | null = null;
let workerHandle: ReturnType<typeof setInterval> | null = null;

async function getDb(): Promise<SQLite.SQLiteDatabase> {
  if (db !== null) return db;
  db = await SQLite.openDatabaseAsync(DB_NAME);
  await db.execAsync(`
    PRAGMA journal_mode = WAL;
    PRAGMA busy_timeout = 5000;
    CREATE TABLE IF NOT EXISTS sync_queue (
      id            TEXT PRIMARY KEY,
      type          TEXT NOT NULL,
      payload_json  TEXT NOT NULL,
      created_at    INTEGER NOT NULL DEFAULT (strftime('%s','now')),
      retry_count   INTEGER NOT NULL DEFAULT 0,
      status        TEXT NOT NULL DEFAULT 'pending',
      last_error    TEXT
    );
    CREATE INDEX IF NOT EXISTS idx_sync_queue_status ON sync_queue(status, created_at);
  `);
  return db;
}

/**
 * UUID v4 light (pas de dépendance externe). Pour usage non-cryptographique.
 */
function uuidv4(): string {
  // RFC 4122 §4.4 — version 4
  const hex = (n: number) => n.toString(16).padStart(2, '0');
  const bytes = new Uint8Array(16);
  for (let i = 0; i < 16; i++) {
    bytes[i] = Math.floor(Math.random() * 256);
  }
  bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
  bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 10
  const b = Array.from(bytes, hex);
  return `${b.slice(0, 4).join('')}-${b.slice(4, 6).join('')}-${b.slice(6, 8).join('')}-${b
    .slice(8, 10)
    .join('')}-${b.slice(10, 16).join('')}`;
}

export async function enqueueSyncItem(item: SyncItem): Promise<string> {
  const conn = await getDb();
  const id = uuidv4();
  await conn.runAsync(
    'INSERT INTO sync_queue (id, type, payload_json, status) VALUES (?, ?, ?, ?)',
    [id, item.type, JSON.stringify(item), 'pending'],
  );
  return id;
}

export async function listPending(limit = BATCH_MAX): Promise<SyncQueueRow[]> {
  const conn = await getDb();
  return conn.getAllAsync<SyncQueueRow>(
    `SELECT * FROM sync_queue
     WHERE status IN ('pending', 'error') AND retry_count < ?
     ORDER BY created_at ASC
     LIMIT ?`,
    [MAX_RETRIES, limit],
  );
}

export async function countByStatus(status: SyncQueueStatus): Promise<number> {
  const conn = await getDb();
  const row = await conn.getFirstAsync<{ c: number }>(
    'SELECT COUNT(*) AS c FROM sync_queue WHERE status = ?',
    [status],
  );
  return row?.c ?? 0;
}

export async function pendingCount(): Promise<number> {
  const conn = await getDb();
  const row = await conn.getFirstAsync<{ c: number }>(
    "SELECT COUNT(*) AS c FROM sync_queue WHERE status IN ('pending', 'error')",
  );
  return row?.c ?? 0;
}

async function markStatus(
  ids: string[],
  status: SyncQueueStatus,
  error?: string,
): Promise<void> {
  if (ids.length === 0) return;
  const conn = await getDb();
  const placeholders = ids.map(() => '?').join(',');
  if (error !== undefined) {
    await conn.runAsync(
      `UPDATE sync_queue SET status = ?, last_error = ?, retry_count = retry_count + 1
       WHERE id IN (${placeholders})`,
      [status, error, ...ids],
    );
  } else {
    await conn.runAsync(
      `UPDATE sync_queue SET status = ? WHERE id IN (${placeholders})`,
      [status, ...ids],
    );
  }
}

/**
 * Tente d'envoyer 1 batch ≤ BATCH_MAX items. Retourne nombre d'items succès.
 */
export async function flushOnce(): Promise<number> {
  const rows = await listPending(BATCH_MAX);
  if (rows.length === 0) return 0;

  const items: SyncItem[] = [];
  const ids: string[] = [];
  for (const row of rows) {
    try {
      const item = JSON.parse(row.payload_json) as SyncItem;
      items.push(item);
      ids.push(row.id);
    } catch {
      // Payload corrompu — marquer en erreur.
      await markStatus([row.id], 'error', 'invalid payload_json');
    }
  }

  if (items.length === 0) return 0;
  await markStatus(ids, 'sending');

  const batchReq: SyncBatchRequest = { batchId: uuidv4(), items };
  let response: SyncBatchResponse;
  try {
    response = await postSyncBatch(batchReq);
  } catch (err) {
    const msg = err instanceof Error ? err.message : 'unknown sync error';
    await markStatus(ids, 'error', msg);
    return 0;
  }

  let okCount = 0;
  for (const ack of response.acks) {
    const id = ids[ack.index];
    if (id === undefined) continue;
    if (ack.status === 'ok') {
      await markStatus([id], 'ok');
      okCount++;
    } else {
      await markStatus([id], 'error', ack.message ?? ack.error ?? 'sync nack');
    }
  }
  return okCount;
}

/**
 * Démarre le worker périodique (idempotent).
 */
export function startSyncWorker(): void {
  if (workerHandle !== null) return;
  workerHandle = setInterval(() => {
    void flushOnce().catch((err) => {
      // Logging silencieux côté UI — visibilité via SyncStatusBanner / Screen.
      // eslint-disable-next-line no-console
      console.warn('[sync-queue] flush failed', err);
    });
  }, WORKER_INTERVAL_MS);
}

export function stopSyncWorker(): void {
  if (workerHandle !== null) {
    clearInterval(workerHandle);
    workerHandle = null;
  }
}

/**
 * Purge les items terminés (status='ok') > 7 jours.
 */
export async function purgeAcked(olderThanSeconds = 7 * 24 * 3600): Promise<number> {
  const conn = await getDb();
  const cutoff = Math.floor(Date.now() / 1000) - olderThanSeconds;
  const result = await conn.runAsync(
    "DELETE FROM sync_queue WHERE status = 'ok' AND created_at < ?",
    [cutoff],
  );
  return result.changes ?? 0;
}
