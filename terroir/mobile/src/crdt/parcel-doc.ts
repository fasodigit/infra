// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Parcel CRDT helper — wrapper Y.Doc pour entité parcelle.
 *
 * Cf. ADR-002 : parcelles = polygone GPS + métadonnées (cultures, surface)
 * → Yjs CRDT (option C Hybrid).
 *
 * Structure du doc :
 * - Y.Map "metadata" : { name, surface_ha, culture, planted_at, ... }
 * - Y.Array "geometry" : [[lat, lng], ...] (polygone fermé)
 * - Y.Array "notes_agent" : Y.XmlFragment (rich text P1)
 *
 * P0 : skeleton. P1 : intégration carte MapLibre + édition polygone.
 */
import * as Y from 'yjs';

import { getDoc } from './yjs-store';

export interface ParcelMetadata {
  name: string;
  surface_ha: number;
  culture: string;
  planted_at?: string; // ISO date
}

export type LatLng = [number, number];

export async function loadParcelDoc(parcelId: string): Promise<Y.Doc> {
  return getDoc(`parcel:${parcelId}`);
}

export function getMetadata(doc: Y.Doc): Y.Map<unknown> {
  return doc.getMap('metadata');
}

export function getGeometry(doc: Y.Doc): Y.Array<LatLng> {
  return doc.getArray<LatLng>('geometry');
}

export function setMetadata(doc: Y.Doc, metadata: Partial<ParcelMetadata>): void {
  const map = getMetadata(doc);
  doc.transact(() => {
    Object.entries(metadata).forEach(([key, value]) => {
      map.set(key, value);
    });
  });
}

export function setGeometry(doc: Y.Doc, points: LatLng[]): void {
  const arr = getGeometry(doc);
  doc.transact(() => {
    arr.delete(0, arr.length);
    arr.insert(0, points);
  });
}
