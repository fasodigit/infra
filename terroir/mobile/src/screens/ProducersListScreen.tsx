// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Liste paginée des producteurs (CompactProducer DTO P1.D).
 *
 * Source : GET /m/producers?cooperativeId=&page=&size=
 * Recherche : filtre client local sur full_name (P3+ : full-text côté BFF).
 * Pull-to-refresh : reload page 0.
 * Click row → ProducerCreateScreen (mode "edit" via route param) — pour
 *             P1 on garde uniquement create ; le mode edit arrive en P2.
 */
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ActivityIndicator,
  FlatList,
  RefreshControl,
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from 'react-native';
import type { NativeStackScreenProps } from '@react-navigation/native-stack';

import { listProducers, type CompactProducer } from '../api/mobile-bff-client';
import { SyncStatusBanner } from '../components/SyncStatusBanner';
import { theme } from '../theme';
import type { RootStackParamList } from '../../App';

type Props = NativeStackScreenProps<RootStackParamList, 'ProducersList'>;

export function ProducersListScreen({ navigation }: Props) {
  const [items, setItems] = useState<CompactProducer[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [refreshing, setRefreshing] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState<string>('');
  const [page, setPage] = useState<number>(0);
  const [hasMore, setHasMore] = useState<boolean>(true);

  const fetchPage = useCallback(async (nextPage: number, replace: boolean) => {
    setError(null);
    try {
      const resp = await listProducers({ page: nextPage, size: 20 });
      setItems((prev) => (replace ? resp.items : [...prev, ...resp.items]));
      setHasMore(resp.items.length === resp.size);
      setPage(nextPage);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Erreur réseau');
    }
  }, []);

  useEffect(() => {
    void (async () => {
      setLoading(true);
      await fetchPage(0, true);
      setLoading(false);
    })();
  }, [fetchPage]);

  async function onRefresh() {
    setRefreshing(true);
    await fetchPage(0, true);
    setRefreshing(false);
  }

  async function onEndReached() {
    if (!hasMore || loading) return;
    await fetchPage(page + 1, false);
  }

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (q.length === 0) return items;
    return items.filter((p) => p.fullName.toLowerCase().includes(q));
  }, [items, query]);

  return (
    <View style={styles.container}>
      <SyncStatusBanner />

      <View style={styles.header}>
        <TextInput
          style={styles.search}
          value={query}
          onChangeText={setQuery}
          placeholder="Rechercher (nom, prénom)…"
          placeholderTextColor="#9e9e9e"
          autoCorrect={false}
        />
        <TouchableOpacity
          style={styles.fab}
          onPress={() => navigation.navigate('ProducerCreate')}
        >
          <Text style={styles.fabText}>+ Nouveau</Text>
        </TouchableOpacity>
      </View>

      {error !== null ? <Text style={styles.errorBanner}>{error}</Text> : null}

      {loading ? (
        <View style={styles.center}>
          <ActivityIndicator color={theme.colors.primary} size="large" />
        </View>
      ) : (
        <FlatList
          data={filtered}
          keyExtractor={(it) => it.id}
          refreshControl={<RefreshControl refreshing={refreshing} onRefresh={onRefresh} />}
          onEndReachedThreshold={0.4}
          onEndReached={() => void onEndReached()}
          ListEmptyComponent={
            <Text style={styles.empty}>Aucun producteur trouvé.</Text>
          }
          renderItem={({ item }) => (
            <TouchableOpacity
              style={styles.row}
              onPress={() =>
                navigation.navigate('ParcelsList', { producerId: item.id })
              }
            >
              <View style={styles.rowMain}>
                <Text style={styles.rowName}>{item.fullName}</Text>
                <Text style={styles.rowMeta}>
                  {item.primaryCrop ?? '—'} ·{' '}
                  {new Date(item.updatedAt).toLocaleDateString('fr-FR')}
                </Text>
              </View>
              <Text style={styles.rowChevron}>›</Text>
            </TouchableOpacity>
          )}
        />
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: theme.colors.background },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    padding: theme.spacing.sm,
    gap: theme.spacing.sm,
    backgroundColor: theme.colors.surface,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  search: {
    flex: 1,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.radius.sm,
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.sm,
    fontSize: theme.fontSize.md,
    backgroundColor: theme.colors.background,
  },
  fab: {
    backgroundColor: theme.colors.primary,
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.sm,
    borderRadius: theme.radius.sm,
  },
  fabText: { color: theme.colors.onPrimary, fontWeight: '600' },
  errorBanner: {
    backgroundColor: theme.colors.error,
    color: '#ffffff',
    padding: theme.spacing.sm,
    textAlign: 'center',
  },
  center: { flex: 1, justifyContent: 'center', alignItems: 'center' },
  empty: { textAlign: 'center', color: '#757575', padding: theme.spacing.lg },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.md,
    backgroundColor: theme.colors.surface,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  rowMain: { flex: 1 },
  rowName: { fontSize: theme.fontSize.lg, fontWeight: '600', color: theme.colors.onBackground },
  rowMeta: { fontSize: theme.fontSize.sm, color: '#757575', marginTop: 2 },
  rowChevron: { fontSize: 28, color: '#9e9e9e' },
});
