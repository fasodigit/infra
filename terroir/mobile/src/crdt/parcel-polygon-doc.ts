// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Wrapper Yjs spécialisé pour le polygone d'une parcelle.
 *
 * Cf. ADR-002 : géom de parcelle = CRDT (Yjs). Stratégie :
 *   - Y.Array<Y.Map<{ lat: number, lng: number }>> pour les sommets
 *     (chaque vertex est un Y.Map → position éditable concurrente).
 *   - Sérialisation pour sync : `Y.encodeStateAsUpdate(doc)` → Uint8Array
 *     → base64 → champ `yjsDelta` du SyncItem (`parcel-polygon-update`).
 *
 * Encoding base64 : pas de Buffer global en RN/Hermes — on utilise un
 * encodeur léger pur-JS (pas de dépendance externe).
 */
import * as Y from 'yjs';

import { getDoc } from './yjs-store';

export interface Vertex {
  lat: number;
  lng: number;
  /** Précision GPS reportée par expo-location, en mètres. */
  accuracy?: number;
}

const GEOMETRY_KEY = 'geometry';

export async function loadParcelPolygonDoc(parcelId: string): Promise<Y.Doc> {
  return getDoc(`parcel:${parcelId}:polygon`);
}

export function getVertices(doc: Y.Doc): Y.Array<Y.Map<unknown>> {
  return doc.getArray<Y.Map<unknown>>(GEOMETRY_KEY);
}

export function appendVertex(doc: Y.Doc, vertex: Vertex): void {
  const arr = getVertices(doc);
  doc.transact(() => {
    const map = new Y.Map<unknown>();
    map.set('lat', vertex.lat);
    map.set('lng', vertex.lng);
    if (vertex.accuracy !== undefined) {
      map.set('accuracy', vertex.accuracy);
    }
    arr.push([map]);
  });
}

export function removeLastVertex(doc: Y.Doc): void {
  const arr = getVertices(doc);
  if (arr.length === 0) return;
  doc.transact(() => {
    arr.delete(arr.length - 1, 1);
  });
}

export function clearVertices(doc: Y.Doc): void {
  const arr = getVertices(doc);
  if (arr.length === 0) return;
  doc.transact(() => {
    arr.delete(0, arr.length);
  });
}

export function readVertices(doc: Y.Doc): Vertex[] {
  const arr = getVertices(doc);
  const out: Vertex[] = [];
  arr.forEach((m: Y.Map<unknown>) => {
    const lat = m.get('lat') as number | undefined;
    const lng = m.get('lng') as number | undefined;
    if (typeof lat === 'number' && typeof lng === 'number') {
      const accuracy = m.get('accuracy') as number | undefined;
      out.push({ lat, lng, accuracy });
    }
  });
  return out;
}

// ---------------------------------------------------------------------------
// Encoding helpers (base64 sans dépendance externe)
// ---------------------------------------------------------------------------

const B64_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

function bytesToBase64(bytes: Uint8Array): string {
  let out = '';
  let i = 0;
  for (; i + 2 < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2];
    out +=
      B64_ALPHABET[(n >> 18) & 0x3f] +
      B64_ALPHABET[(n >> 12) & 0x3f] +
      B64_ALPHABET[(n >> 6) & 0x3f] +
      B64_ALPHABET[n & 0x3f];
  }
  if (i < bytes.length) {
    const a = bytes[i];
    const b = i + 1 < bytes.length ? bytes[i + 1] : 0;
    const n = (a << 16) | (b << 8);
    out += B64_ALPHABET[(n >> 18) & 0x3f] + B64_ALPHABET[(n >> 12) & 0x3f];
    out += i + 1 < bytes.length ? B64_ALPHABET[(n >> 6) & 0x3f] : '=';
    out += '=';
  }
  return out;
}

function base64ToBytes(b64: string): Uint8Array {
  const clean = b64.replace(/[^A-Za-z0-9+/=]/g, '');
  const len = clean.length;
  const padding = clean.endsWith('==') ? 2 : clean.endsWith('=') ? 1 : 0;
  const byteLen = (len * 3) / 4 - padding;
  const out = new Uint8Array(byteLen);
  let p = 0;
  for (let i = 0; i < len; i += 4) {
    const a = B64_ALPHABET.indexOf(clean[i]);
    const b = B64_ALPHABET.indexOf(clean[i + 1]);
    const c = clean[i + 2] === '=' ? 0 : B64_ALPHABET.indexOf(clean[i + 2]);
    const d = clean[i + 3] === '=' ? 0 : B64_ALPHABET.indexOf(clean[i + 3]);
    const n = (a << 18) | (b << 12) | (c << 6) | d;
    if (p < byteLen) out[p++] = (n >> 16) & 0xff;
    if (p < byteLen) out[p++] = (n >> 8) & 0xff;
    if (p < byteLen) out[p++] = n & 0xff;
  }
  return out;
}

/**
 * Encode l'état complet du doc en base64 (pour `yjsDelta` du SyncItem).
 */
export function encodeDocAsB64(doc: Y.Doc): string {
  const update = Y.encodeStateAsUpdate(doc);
  return bytesToBase64(update);
}

/**
 * Calcule le delta entre `doc` et un state vector remote (server-known).
 * Si `remoteStateVectorB64` est null, encode l'état complet.
 */
export function encodeDeltaAsB64(doc: Y.Doc, remoteStateVectorB64?: string): string {
  if (!remoteStateVectorB64) return encodeDocAsB64(doc);
  const sv = base64ToBytes(remoteStateVectorB64);
  const update = Y.encodeStateAsUpdate(doc, sv);
  return bytesToBase64(update);
}

export function applyB64Update(doc: Y.Doc, b64Update: string): void {
  const bytes = base64ToBytes(b64Update);
  Y.applyUpdate(doc, bytes);
}

/**
 * Convertit le polygone courant en chaîne WKT (POLYGON((lng lat, ...))).
 * Pratique pour matérialiser dans la table SQLite locale ou pour debug.
 */
export function verticesToWkt(vertices: Vertex[]): string | null {
  if (vertices.length < 3) return null;
  const ring = [...vertices, vertices[0]]
    .map((v) => `${v.lng} ${v.lat}`)
    .join(', ');
  return `POLYGON((${ring}))`;
}
