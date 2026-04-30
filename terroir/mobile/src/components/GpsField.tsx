// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * GpsField — composant UI qui demande la permission GPS, lit la position
 * (precision High) via `expo-location`, affiche lat/lng + précision.
 *
 * Props :
 *   - onChange(coords) : remontée de la position lue
 *   - autoFetch       : si true, fetch au mount
 */
import { useEffect, useState } from 'react';
import { ActivityIndicator, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import * as Location from 'expo-location';

import { theme } from '../theme';

export interface GpsCoords {
  latitude: number;
  longitude: number;
  accuracy?: number | null;
}

interface Props {
  label?: string;
  onChange?: (coords: GpsCoords | null) => void;
  autoFetch?: boolean;
}

export function GpsField({ label = 'Position GPS', onChange, autoFetch = false }: Props) {
  const [coords, setCoords] = useState<GpsCoords | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function fetchPosition() {
    setLoading(true);
    setError(null);
    try {
      const { status } = await Location.requestForegroundPermissionsAsync();
      if (status !== 'granted') {
        setError('Permission GPS refusée.');
        onChange?.(null);
        return;
      }
      const pos = await Location.getCurrentPositionAsync({
        accuracy: Location.Accuracy.High,
      });
      const c: GpsCoords = {
        latitude: pos.coords.latitude,
        longitude: pos.coords.longitude,
        accuracy: pos.coords.accuracy,
      };
      setCoords(c);
      onChange?.(c);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Erreur GPS');
      onChange?.(null);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (autoFetch) {
      void fetchPosition();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoFetch]);

  return (
    <View style={styles.container}>
      <Text style={styles.label}>{label}</Text>
      <View style={styles.row}>
        <View style={styles.values}>
          {loading ? (
            <ActivityIndicator color={theme.colors.primary} />
          ) : coords !== null ? (
            <>
              <Text style={styles.value}>
                {coords.latitude.toFixed(6)}, {coords.longitude.toFixed(6)}
              </Text>
              {typeof coords.accuracy === 'number' ? (
                <Text style={styles.muted}>± {coords.accuracy.toFixed(1)} m</Text>
              ) : null}
            </>
          ) : (
            <Text style={styles.muted}>Aucune position acquise.</Text>
          )}
          {error !== null ? <Text style={styles.error}>{error}</Text> : null}
        </View>
        <TouchableOpacity
          style={styles.button}
          onPress={() => void fetchPosition()}
          disabled={loading}
        >
          <Text style={styles.buttonText}>Capturer</Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    marginBottom: theme.spacing.md,
  },
  label: {
    fontSize: theme.fontSize.md,
    fontWeight: '600',
    marginBottom: theme.spacing.xs,
    color: theme.colors.onBackground,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: theme.colors.surface,
    borderRadius: theme.radius.sm,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing.sm,
  },
  values: {
    flex: 1,
  },
  value: {
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
    fontFamily: 'monospace',
  },
  muted: {
    fontSize: theme.fontSize.sm,
    color: '#757575',
  },
  error: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.error,
    marginTop: theme.spacing.xs,
  },
  button: {
    backgroundColor: theme.colors.primary,
    paddingVertical: theme.spacing.sm,
    paddingHorizontal: theme.spacing.md,
    borderRadius: theme.radius.sm,
  },
  buttonText: {
    color: theme.colors.onPrimary,
    fontWeight: '600',
  },
});
