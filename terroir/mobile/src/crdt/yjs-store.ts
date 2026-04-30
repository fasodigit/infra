// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Yjs store TERROIR — registre central des Y.Doc.
 *
 * Cf. ADR-002 : Yjs (option Hybrid C, choix CRDT) pour parcelles + profil
 * producteur étendu. Pas Automerge — décision User TERROIR P0.G.
 *
 * Pattern :
 * - Chaque entité offline-éditable a un Y.Doc identifié par `docName`
 *   (ex : `parcel:${uuid}`, `producer:${uuid}/notes`).
 * - L'adapter SQLite (sqlite-adapter.ts) persiste les updates incrémentaux
 *   et permet un load lazy.
 * - Sync : updates Yjs encodés (Uint8Array) envoyés au mobile-bff via
 *   POST /sync/yjs/{docName} → broadcast aux autres clients (websocket P1).
 */
import * as Y from 'yjs';

import { SqliteYjsAdapter } from './sqlite-adapter';

const docs = new Map<string, Y.Doc>();
let adapter: SqliteYjsAdapter | null = null;

export async function initYjsStore(): Promise<void> {
  if (adapter !== null) {
    return;
  }
  adapter = new SqliteYjsAdapter();
  await adapter.init();
}

export async function getDoc(docName: string): Promise<Y.Doc> {
  if (adapter === null) {
    await initYjsStore();
  }
  let doc = docs.get(docName);
  if (doc !== undefined) {
    return doc;
  }
  doc = new Y.Doc();
  // Hydrate depuis SQLite si snapshot existant.
  const snapshot = await adapter!.loadDoc(docName);
  if (snapshot !== null) {
    Y.applyUpdate(doc, snapshot);
  }
  // Persiste chaque update local (debounce P1).
  doc.on('update', (update: Uint8Array, _origin: unknown) => {
    void adapter!.appendUpdate(docName, update);
  });
  docs.set(docName, doc);
  return doc;
}

export async function destroyDoc(docName: string): Promise<void> {
  const doc = docs.get(docName);
  if (doc !== undefined) {
    doc.destroy();
    docs.delete(docName);
  }
  if (adapter !== null) {
    await adapter.deleteDoc(docName);
  }
}

export async function listDocs(): Promise<string[]> {
  if (adapter === null) {
    await initYjsStore();
  }
  return adapter!.listDocs();
}
