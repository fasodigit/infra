// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * SyncStatusBanner — bandeau persistant en haut des écrans applicatifs.
 *
 * États affichés :
 *   - "À jour" (vert)         si pendingCount = 0 ET network OK
 *   - "X items en attente"    si pendingCount > 0
 *   - "Sync en cours..."      pendant flushOnce()
 *   - "Hors ligne"            si expo-network indique disconnected
 *
 * Refresh : toutes les 5 secondes (lecture compteur SQLite, peu coûteuse).
 */
import { useEffect, useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import * as Network from 'expo-network';

import { pendingCount } from '../api/sync-queue';
import { theme } from '../theme';

type State = 'up_to_date' | 'pending' | 'syncing' | 'offline';

interface Props {
  syncing?: boolean;
  pollingMs?: number;
}

export function SyncStatusBanner({ syncing = false, pollingMs = 5_000 }: Props) {
  const [count, setCount] = useState<number>(0);
  const [online, setOnline] = useState<boolean>(true);

  useEffect(() => {
    let cancelled = false;
    async function refresh() {
      try {
        const [c, n] = await Promise.all([pendingCount(), Network.getNetworkStateAsync()]);
        if (!cancelled) {
          setCount(c);
          setOnline(Boolean(n.isInternetReachable ?? n.isConnected));
        }
      } catch {
        // ignore
      }
    }
    void refresh();
    const handle = setInterval(refresh, pollingMs);
    return () => {
      cancelled = true;
      clearInterval(handle);
    };
  }, [pollingMs]);

  const state: State = !online
    ? 'offline'
    : syncing
      ? 'syncing'
      : count > 0
        ? 'pending'
        : 'up_to_date';

  const colors: Record<State, string> = {
    up_to_date: theme.colors.success,
    pending: theme.colors.warning,
    syncing: theme.colors.primary,
    offline: '#757575',
  };

  const labels: Record<State, string> = {
    up_to_date: 'À jour',
    pending: `${count} item${count > 1 ? 's' : ''} en attente`,
    syncing: 'Sync en cours…',
    offline: 'Hors ligne',
  };

  return (
    <View style={[styles.container, { backgroundColor: colors[state] }]}>
      <Text style={styles.text}>{labels[state]}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.xs,
    alignItems: 'center',
  },
  text: {
    color: '#ffffff',
    fontWeight: '600',
    fontSize: theme.fontSize.sm,
  },
});
