// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * PolygonDrawer — composant carte MapLibre avec tap-to-add-vertex.
 *
 * Souveraineté : MapLibre (fork OSS de Mapbox GL) + tiles OSM. PAS de
 * Google Maps SDK, pas de Mapbox commercial. Cf. CLAUDE.md §3.
 *
 * Comportement :
 *   - chaque tap sur la carte → ajoute un vertex au polygone (Yjs Doc).
 *   - undo / clear via boutons internes.
 *   - props : initialVertices, onChange (callback à chaque mutation).
 *
 * Note RN : `@maplibre/maplibre-react-native` peut ne pas être installable
 * en dev sans build natif Android/iOS — l'écran retombe sur un fallback
 * "skeleton" si le module est absent (try/catch require).
 */
import { useMemo, useState } from 'react';
import { Alert, StyleSheet, Text, TouchableOpacity, View } from 'react-native';

import { theme } from '../theme';
import type { Vertex } from '../crdt/parcel-polygon-doc';

// Import "lazy" pour ne pas planter en dev si module natif absent.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let MapLibre: any = null;
try {
  // eslint-disable-next-line @typescript-eslint/no-require-imports, @typescript-eslint/no-var-requires
  MapLibre = require('@maplibre/maplibre-react-native');
} catch {
  MapLibre = null;
}

const OSM_STYLE_URL =
  'https://tiles.osm.org/styles/openstreetmap-vector.json'; // P2 : self-hosted OSM tile server

interface Props {
  initialVertices?: Vertex[];
  centerLatLng?: [number, number]; // [lng, lat] format MapLibre
  onChange?: (vertices: Vertex[]) => void;
}

export function PolygonDrawer({
  initialVertices = [],
  centerLatLng = [-1.5197, 12.3714], // Ouagadougou
  onChange,
}: Props) {
  const [vertices, setVertices] = useState<Vertex[]>(initialVertices);

  const polygonGeoJson = useMemo(() => {
    if (vertices.length < 3) return null;
    return {
      type: 'Feature' as const,
      properties: {},
      geometry: {
        type: 'Polygon' as const,
        coordinates: [
          [...vertices.map((v) => [v.lng, v.lat]), [vertices[0].lng, vertices[0].lat]],
        ],
      },
    };
  }, [vertices]);

  function addVertex(lng: number, lat: number) {
    const next = [...vertices, { lat, lng }];
    setVertices(next);
    onChange?.(next);
  }

  function undoVertex() {
    if (vertices.length === 0) return;
    const next = vertices.slice(0, -1);
    setVertices(next);
    onChange?.(next);
  }

  function clearAll() {
    setVertices([]);
    onChange?.([]);
  }

  if (MapLibre === null) {
    return (
      <View style={styles.fallback}>
        <Text style={styles.fallbackText}>
          Carte indisponible (module @maplibre/maplibre-react-native non chargé).{'\n'}
          Sommets actuels : {vertices.length}
        </Text>
        <View style={styles.controls}>
          <TouchableOpacity
            style={styles.button}
            onPress={() => addVertex(centerLatLng[0], centerLatLng[1])}
          >
            <Text style={styles.buttonText}>Ajouter sommet (centre)</Text>
          </TouchableOpacity>
          <TouchableOpacity style={styles.buttonSecondary} onPress={undoVertex}>
            <Text style={styles.buttonSecondaryText}>Annuler dernier</Text>
          </TouchableOpacity>
        </View>
      </View>
    );
  }

  const { MapView, ShapeSource, FillLayer, LineLayer, CircleLayer, Camera } = MapLibre;

  return (
    <View style={styles.container}>
      <MapView
        style={styles.map}
        styleURL={OSM_STYLE_URL}
        onPress={(e: { geometry: { coordinates: [number, number] } }) => {
          const [lng, lat] = e.geometry.coordinates;
          addVertex(lng, lat);
        }}
      >
        <Camera defaultSettings={{ centerCoordinate: centerLatLng, zoomLevel: 15 }} />

        {polygonGeoJson !== null ? (
          <ShapeSource id="polygon-src" shape={polygonGeoJson}>
            <FillLayer id="polygon-fill" style={{ fillColor: '#1b5e20', fillOpacity: 0.3 }} />
            <LineLayer
              id="polygon-line"
              style={{ lineColor: '#1b5e20', lineWidth: 2 }}
            />
          </ShapeSource>
        ) : null}

        <ShapeSource
          id="vertices-src"
          shape={{
            type: 'FeatureCollection',
            features: vertices.map((v, i) => ({
              type: 'Feature',
              properties: { idx: i },
              geometry: { type: 'Point', coordinates: [v.lng, v.lat] },
            })),
          }}
        >
          <CircleLayer
            id="vertices-dots"
            style={{
              circleRadius: 6,
              circleColor: '#f9a825',
              circleStrokeColor: '#1b5e20',
              circleStrokeWidth: 2,
            }}
          />
        </ShapeSource>
      </MapView>

      <View style={styles.controls}>
        <Text style={styles.counter}>Sommets : {vertices.length}</Text>
        <TouchableOpacity style={styles.buttonSecondary} onPress={undoVertex}>
          <Text style={styles.buttonSecondaryText}>Undo</Text>
        </TouchableOpacity>
        <TouchableOpacity
          style={styles.buttonDanger}
          onPress={() =>
            Alert.alert('Effacer le tracé ?', 'Cette action est irréversible.', [
              { text: 'Annuler', style: 'cancel' },
              { text: 'Effacer', style: 'destructive', onPress: clearAll },
            ])
          }
        >
          <Text style={styles.buttonText}>Effacer</Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  map: {
    flex: 1,
    minHeight: 300,
  },
  fallback: {
    backgroundColor: theme.colors.surface,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.radius.md,
    padding: theme.spacing.md,
    minHeight: 200,
    justifyContent: 'center',
  },
  fallbackText: {
    color: theme.colors.onBackground,
    textAlign: 'center',
    marginBottom: theme.spacing.md,
  },
  controls: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: theme.spacing.sm,
    padding: theme.spacing.sm,
    backgroundColor: theme.colors.surface,
  },
  counter: {
    flex: 1,
    fontSize: theme.fontSize.md,
    fontWeight: '600',
    color: theme.colors.onBackground,
  },
  button: {
    backgroundColor: theme.colors.primary,
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.sm,
    borderRadius: theme.radius.sm,
  },
  buttonText: {
    color: theme.colors.onPrimary,
    fontWeight: '600',
  },
  buttonSecondary: {
    backgroundColor: theme.colors.surface,
    borderWidth: 1,
    borderColor: theme.colors.primary,
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.sm,
    borderRadius: theme.radius.sm,
  },
  buttonSecondaryText: {
    color: theme.colors.primary,
    fontWeight: '600',
  },
  buttonDanger: {
    backgroundColor: theme.colors.error,
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.sm,
    borderRadius: theme.radius.sm,
  },
});
