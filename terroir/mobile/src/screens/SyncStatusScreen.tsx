// SPDX-License-Identifier: AGPL-3.0-or-later
import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { theme } from '../theme';

export function SyncStatusScreen() {
  // TODO P1 : brancher sur yjs-store.listDocs() + status BFF /sync/health.
  const mockState = {
    last_sync_at: null as number | null,
    pending_uploads: 0,
    pending_downloads: 0,
    conflicts: 0,
    docs: [] as string[],
  };

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.title}>État de la synchronisation</Text>

      <View style={styles.row}>
        <Text style={styles.label}>Dernière sync :</Text>
        <Text style={styles.value}>
          {mockState.last_sync_at !== null
            ? new Date(mockState.last_sync_at).toLocaleString('fr-FR')
            : 'jamais'}
        </Text>
      </View>

      <View style={styles.row}>
        <Text style={styles.label}>Uploads en attente :</Text>
        <Text style={styles.value}>{mockState.pending_uploads}</Text>
      </View>

      <View style={styles.row}>
        <Text style={styles.label}>Downloads en attente :</Text>
        <Text style={styles.value}>{mockState.pending_downloads}</Text>
      </View>

      <View style={styles.row}>
        <Text style={styles.label}>Conflits CRDT :</Text>
        <Text style={styles.value}>{mockState.conflicts}</Text>
      </View>

      <Text style={styles.section}>Documents Yjs locaux</Text>
      {mockState.docs.length === 0 ? (
        <Text style={styles.muted}>Aucun document local pour l&apos;instant.</Text>
      ) : (
        mockState.docs.map((doc) => (
          <Text key={doc} style={styles.value}>
            - {doc}
          </Text>
        ))
      )}

      <Text style={styles.footer}>
        {/* TODO P1 (cf. ADR-002) : afficher conflits, log audit, replay. */}
        Synchronisation effective implémentée en P1.6.
      </Text>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: theme.colors.background,
  },
  content: {
    padding: theme.spacing.md,
  },
  title: {
    fontSize: theme.fontSize.xl,
    fontWeight: '700',
    color: theme.colors.primary,
    marginBottom: theme.spacing.lg,
  },
  row: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    paddingVertical: theme.spacing.sm,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  label: {
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
    fontWeight: '500',
  },
  value: {
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
  },
  section: {
    fontSize: theme.fontSize.lg,
    fontWeight: '600',
    color: theme.colors.onBackground,
    marginTop: theme.spacing.lg,
    marginBottom: theme.spacing.sm,
  },
  muted: {
    fontSize: theme.fontSize.md,
    color: '#757575',
    fontStyle: 'italic',
  },
  footer: {
    marginTop: theme.spacing.xl,
    fontSize: theme.fontSize.sm,
    color: '#757575',
    textAlign: 'center',
  },
});
