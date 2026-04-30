// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Adapter Yjs <-> expo-sqlite.
 *
 * y-indexeddb ne fonctionne pas en RN (pas d'IndexedDB). On implémente
 * donc un adapter custom qui sérialise les updates Yjs (Uint8Array,
 * encodé base64) dans une table SQLite locale.
 *
 * Schéma :
 * ```sql
 * CREATE TABLE IF NOT EXISTS yjs_doc (
 *   doc_name      TEXT NOT NULL,
 *   seq           INTEGER NOT NULL,
 *   update_blob   BLOB NOT NULL,
 *   created_at    INTEGER NOT NULL DEFAULT (strftime('%s','now')),
 *   PRIMARY KEY (doc_name, seq)
 * );
 * CREATE INDEX IF NOT EXISTS idx_yjs_doc_name ON yjs_doc(doc_name);
 * ```
 *
 * Stratégie de compaction :
 * - Au-delà de N=200 updates pour un doc, fusionner via `Y.mergeUpdates`
 *   et remplacer par un seul snapshot (seq=0).
 * - Job background P1 (15min) — voir ADR-002 §Conséquences.
 *
 * TODO P1 :
 * - [ ] Implémentation complète des méthodes (init / appendUpdate /
 *       loadDoc / deleteDoc / listDocs / compact).
 * - [ ] Tests property-based (proptest-like avec fast-check) pour
 *       garantir convergence après random updates.
 * - [ ] Chiffrement at-rest des blob (DEK Vault Transit, cf. ADR-005).
 * - [ ] WAL mode SQLite + busy_timeout pour concurrence sync background.
 */
import * as SQLite from 'expo-sqlite';
import * as Y from 'yjs';

const DB_NAME = 'terroir-yjs.db';
const COMPACTION_THRESHOLD = 200;

export class SqliteYjsAdapter {
  private db: SQLite.SQLiteDatabase | null = null;

  async init(): Promise<void> {
    if (this.db !== null) {
      return;
    }
    this.db = await SQLite.openDatabaseAsync(DB_NAME);
    await this.db.execAsync(`
      PRAGMA journal_mode = WAL;
      PRAGMA busy_timeout = 5000;
      CREATE TABLE IF NOT EXISTS yjs_doc (
        doc_name      TEXT NOT NULL,
        seq           INTEGER NOT NULL,
        update_blob   BLOB NOT NULL,
        created_at    INTEGER NOT NULL DEFAULT (strftime('%s','now')),
        PRIMARY KEY (doc_name, seq)
      );
      CREATE INDEX IF NOT EXISTS idx_yjs_doc_name ON yjs_doc(doc_name);
    `);
  }

  /**
   * Append un update Yjs (Uint8Array) pour le doc donné.
   * Auto-compacte au-delà du seuil COMPACTION_THRESHOLD.
   *
   * TODO P1 : implémenter (signature stable, body placeholder).
   */
  async appendUpdate(docName: string, update: Uint8Array): Promise<void> {
    this.assertReady();
    // TODO P1 : INSERT INTO yjs_doc (doc_name, seq, update_blob)
    //           VALUES (?, COALESCE(MAX(seq)+1, 0), ?)
    // puis compact si count > threshold.
    void docName;
    void update;
  }

  /**
   * Charge tous les updates d'un doc et les fusionne en un seul Uint8Array
   * via Y.mergeUpdates. Retourne null si doc absent.
   *
   * TODO P1 : implémenter.
   */
  async loadDoc(docName: string): Promise<Uint8Array | null> {
    this.assertReady();
    // TODO P1 : SELECT update_blob FROM yjs_doc WHERE doc_name=? ORDER BY seq
    //           → Y.mergeUpdates(rows.map(decode))
    void docName;
    return null;
  }

  async deleteDoc(docName: string): Promise<void> {
    this.assertReady();
    await this.db!.runAsync('DELETE FROM yjs_doc WHERE doc_name = ?', [docName]);
  }

  async listDocs(): Promise<string[]> {
    this.assertReady();
    const rows = await this.db!.getAllAsync<{ doc_name: string }>(
      'SELECT DISTINCT doc_name FROM yjs_doc ORDER BY doc_name',
    );
    return rows.map((r) => r.doc_name);
  }

  /**
   * Compacte les updates en un snapshot unique pour un doc donné.
   * TODO P1 : implémenter.
   */
  async compact(docName: string): Promise<void> {
    this.assertReady();
    // TODO P1 :
    // 1. SELECT update_blob ORDER BY seq
    // 2. merged = Y.mergeUpdates(updates)
    // 3. DELETE FROM yjs_doc WHERE doc_name=?
    // 4. INSERT (doc_name, 0, merged)
    void docName;
    void Y; // ref pour TS lint
    void COMPACTION_THRESHOLD;
  }

  private assertReady(): void {
    if (this.db === null) {
      throw new Error('SqliteYjsAdapter not initialized. Call init() first.');
    }
  }
}
