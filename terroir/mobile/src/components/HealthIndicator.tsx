// SPDX-License-Identifier: AGPL-3.0-or-later
import { useEffect, useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { apiClient } from '../api/client';
import type { HealthResponse } from '../api/types';
import { theme } from '../theme';

type Status = 'unknown' | 'up' | 'degraded' | 'down';

export function HealthIndicator() {
  const [status, setStatus] = useState<Status>('unknown');
  const [version, setVersion] = useState<string>('');

  useEffect(() => {
    let cancelled = false;
    async function probe() {
      try {
        const health = await apiClient.get<HealthResponse>('/health');
        if (!cancelled) {
          setStatus(health.status);
          setVersion(health.version);
        }
      } catch {
        if (!cancelled) {
          setStatus('down');
        }
      }
    }
    void probe();
    const interval = setInterval(probe, 30_000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  const colorByStatus: Record<Status, string> = {
    unknown: '#9e9e9e',
    up: theme.colors.success,
    degraded: theme.colors.warning,
    down: theme.colors.error,
  };

  const labelByStatus: Record<Status, string> = {
    unknown: 'Vérification...',
    up: 'BFF accessible',
    degraded: 'BFF dégradé',
    down: 'Hors ligne',
  };

  return (
    <View style={styles.container}>
      <View style={[styles.dot, { backgroundColor: colorByStatus[status] }]} />
      <Text style={styles.label}>{labelByStatus[status]}</Text>
      {version.length > 0 ? <Text style={styles.version}>v{version}</Text> : null}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: theme.spacing.sm,
    paddingHorizontal: theme.spacing.md,
    backgroundColor: theme.colors.surface,
    borderRadius: theme.radius.sm,
    marginBottom: theme.spacing.md,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  dot: {
    width: 12,
    height: 12,
    borderRadius: 6,
    marginRight: theme.spacing.sm,
  },
  label: {
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
    flex: 1,
  },
  version: {
    fontSize: theme.fontSize.sm,
    color: '#757575',
  },
});
