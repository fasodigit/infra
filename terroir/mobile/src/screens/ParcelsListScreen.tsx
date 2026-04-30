// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Liste des parcelles d'un producteur sélectionné + carte MapLibre.
 *
 * Source : GET /m/parcels?producerId=&page=&size=
 * Carte : MapLibre RN avec marqueurs centroïdes (calculés client à partir
 *         du WKT polygon — fallback simple : barycentre points polygon).
 *
 * Props route : producerId (UUID)
 */
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ActivityIndicator,
  FlatList,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from 'react-native';
import type { NativeStackScreenProps } from '@react-navigation/native-stack';

import { listParcels, type CompactParcel } from '../api/mobile-bff-client';
import { SyncStatusBanner } from '../components/SyncStatusBanner';
import { theme } from '../theme';
import type { RootStackParamList } from '../../App';

// Lazy-load MapLibre (idem PolygonDrawer) pour ne pas planter en l'absence
// du module natif côté dev.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let MapLibre: any = null;
try {
  // eslint-disable-next-line @typescript-eslint/no-require-imports, @typescript-eslint/no-var-requires
  MapLibre = require('@maplibre/maplibre-react-native');
} catch {
  MapLibre = null;
}

const OSM_STYLE_URL = 'https://tiles.osm.org/styles/openstreetmap-vector.json';

type Props = NativeStackScreenProps<RootStackParamList, 'ParcelsList'>;

interface Centroid {
  parcelId: string;
  lat: number;
  lng: number;
  cropType?: string;
}

/**
 * Parse un WKT POLYGON((lng lat, lng lat, ...)) → [lng, lat][] pour ring 0.
 * Pas de support MultiPolygon ici (P1 — 1 polygone par parcelle suffit).
 */
function parseWktPolygon(wkt: string): [number, number][] {
  const m = wkt.match(/POLYGON\s*\(\s*\(([^)]+)\)/i);
  if (m === null) return [];
  return m[1]
    .split(',')
    .map((s) => s.trim().split(/\s+/).map(Number))
    .filter((arr) => arr.length === 2 && !arr.some(Number.isNaN))
    .map(([lng, lat]) => [lng, lat] as [number, number]);
}

function centroid(coords: [number, number][]): { lng: number; lat: number } | null {
  if (coords.length === 0) return null;
  let lngSum = 0;
  let latSum = 0;
  for (const [lng, lat] of coords) {
    lngSum += lng;
    latSum += lat;
  }
  return { lng: lngSum / coords.length, lat: latSum / coords.length };
}

export function ParcelsListScreen({ navigation, route }: Props) {
  const producerId = route.params?.producerId;
  const [items, setItems] = useState<CompactParcel[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!producerId) {
      setLoading(false);
      return;
    }
    setError(null);
    try {
      const resp = await listParcels({ producerId, page: 0, size: 100 });
      setItems(resp.items);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Erreur réseau');
    } finally {
      setLoading(false);
    }
  }, [producerId]);

  useEffect(() => {
    void fetchData();
  }, [fetchData]);

  const centroids = useMemo<Centroid[]>(() => {
    const out: Centroid[] = [];
    for (const p of items) {
      if (!p.geomWkt) continue;
      const ring = parseWktPolygon(p.geomWkt);
      const c = centroid(ring);
      if (c === null) continue;
      out.push({ parcelId: p.id, lat: c.lat, lng: c.lng, cropType: p.cropType });
    }
    return out;
  }, [items]);

  const initialCenter = useMemo<[number, number]>(() => {
    if (centroids.length > 0) return [centroids[0].lng, centroids[0].lat];
    return [-1.5197, 12.3714]; // Ouagadougou
  }, [centroids]);

  return (
    <View style={styles.container}>
      <SyncStatusBanner />

      <View style={styles.headerBar}>
        <Text style={styles.headerTitle}>Parcelles</Text>
        <TouchableOpacity
          style={styles.fab}
          onPress={() =>
            navigation.navigate('ParcelCreate', { producerId: producerId ?? '' })
          }
        >
          <Text style={styles.fabText}>+ Tracer</Text>
        </TouchableOpacity>
      </View>

      <View style={styles.mapWrap}>
        {MapLibre !== null ? (
          <MapLibre.MapView style={styles.map} styleURL={OSM_STYLE_URL}>
            <MapLibre.Camera
              defaultSettings={{ centerCoordinate: initialCenter, zoomLevel: 13 }}
            />
            <MapLibre.ShapeSource
              id="parcels-centroids"
              shape={{
                type: 'FeatureCollection',
                features: centroids.map((c) => ({
                  type: 'Feature',
                  properties: { parcelId: c.parcelId, cropType: c.cropType ?? '' },
                  geometry: { type: 'Point', coordinates: [c.lng, c.lat] },
                })),
              }}
            >
              <MapLibre.CircleLayer
                id="parcels-centroids-dots"
                style={{
                  circleRadius: 8,
                  circleColor: theme.colors.primary,
                  circleStrokeColor: '#ffffff',
                  circleStrokeWidth: 2,
                }}
              />
            </MapLibre.ShapeSource>
          </MapLibre.MapView>
        ) : (
          <View style={styles.mapFallback}>
            <Text style={styles.mapFallbackText}>
              Carte indisponible.{'\n'}
              {centroids.length} centroïde(s) calculé(s).
            </Text>
          </View>
        )}
      </View>

      {error !== null ? <Text style={styles.errorBanner}>{error}</Text> : null}

      {loading ? (
        <View style={styles.center}>
          <ActivityIndicator color={theme.colors.primary} size="large" />
        </View>
      ) : (
        <FlatList
          data={items}
          keyExtractor={(it) => it.id}
          ListEmptyComponent={
            <Text style={styles.empty}>Aucune parcelle pour ce producteur.</Text>
          }
          renderItem={({ item }) => (
            <View style={styles.row}>
              <View style={styles.rowMain}>
                <Text style={styles.rowName}>
                  {item.cropType ?? 'Culture inconnue'}
                </Text>
                <Text style={styles.rowMeta}>
                  {item.surfaceHectares
                    ? `${item.surfaceHectares.toFixed(2)} ha`
                    : 'surface inconnue'}{' '}
                  · {new Date(item.updatedAt).toLocaleDateString('fr-FR')}
                </Text>
              </View>
            </View>
          )}
        />
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: theme.colors.background },
  headerBar: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.sm,
    backgroundColor: theme.colors.surface,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  headerTitle: { flex: 1, fontSize: theme.fontSize.lg, fontWeight: '700', color: theme.colors.primary },
  fab: {
    backgroundColor: theme.colors.primary,
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.sm,
    borderRadius: theme.radius.sm,
  },
  fabText: { color: theme.colors.onPrimary, fontWeight: '600' },
  mapWrap: { height: 240, backgroundColor: '#eeeeee' },
  map: { flex: 1 },
  mapFallback: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: theme.spacing.md,
  },
  mapFallbackText: { color: '#757575', textAlign: 'center' },
  errorBanner: {
    backgroundColor: theme.colors.error,
    color: '#ffffff',
    padding: theme.spacing.sm,
    textAlign: 'center',
  },
  center: { flex: 1, justifyContent: 'center', alignItems: 'center' },
  empty: { textAlign: 'center', color: '#757575', padding: theme.spacing.lg },
  row: {
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.md,
    backgroundColor: theme.colors.surface,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  rowMain: {},
  rowName: { fontSize: theme.fontSize.lg, fontWeight: '600', color: theme.colors.onBackground },
  rowMeta: { fontSize: theme.fontSize.sm, color: '#757575', marginTop: 2 },
});
