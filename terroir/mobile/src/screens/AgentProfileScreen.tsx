// SPDX-License-Identifier: AGPL-3.0-or-later
import { useEffect, useState } from 'react';
import { Alert, ScrollView, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import type { NativeStackScreenProps } from '@react-navigation/native-stack';

import { logout } from '../auth/kratos-flow';
import { whoami } from '../auth/kratos-client';
import type { AuthSession } from '../api/types';
import { pendingCount } from '../api/sync-queue';
import { SyncStatusBanner } from '../components/SyncStatusBanner';
import { theme } from '../theme';
import type { RootStackParamList } from '../../App';

type Props = NativeStackScreenProps<RootStackParamList, 'AgentProfile'>;

export function AgentProfileScreen({ navigation }: Props) {
  const [session, setSession] = useState<AuthSession | null>(null);
  const [pending, setPending] = useState<number>(0);
  const [lastSync, setLastSync] = useState<number | null>(null);

  useEffect(() => {
    void (async () => {
      const s = await whoami();
      setSession(s);
      const c = await pendingCount();
      setPending(c);
      // TODO P2 : last_sync_at depuis KAYA via /m/sync/status (P1.D extend).
      setLastSync(null);
    })();
  }, []);

  async function handleLogout() {
    Alert.alert('Déconnexion', 'Confirmer la déconnexion ?', [
      { text: 'Annuler', style: 'cancel' },
      {
        text: 'Déconnexion',
        style: 'destructive',
        onPress: async () => {
          await logout();
          navigation.replace('Login');
        },
      },
    ]);
  }

  return (
    <View style={styles.container}>
      <SyncStatusBanner />
      <ScrollView contentContainerStyle={styles.content}>
        <Text style={styles.title}>Profil agent</Text>

        <View style={styles.card}>
          <Text style={styles.label}>Identifiant agent</Text>
          <Text style={styles.value}>{session?.agent_id ?? '—'}</Text>

          <Text style={styles.label}>Coopérative active (tenant)</Text>
          <Text style={styles.value}>{session?.tenant_id ?? '—'}</Text>

          <Text style={styles.label}>Session valide jusqu&apos;au</Text>
          <Text style={styles.value}>
            {session?.expires_at
              ? new Date(session.expires_at * 1000).toLocaleString('fr-FR')
              : '—'}
          </Text>
        </View>

        <View style={styles.card}>
          <Text style={styles.cardTitle}>Synchronisation</Text>
          <View style={styles.row}>
            <Text style={styles.label}>Dernière sync</Text>
            <Text style={styles.value}>
              {lastSync !== null ? new Date(lastSync).toLocaleString('fr-FR') : 'jamais'}
            </Text>
          </View>
          <View style={styles.row}>
            <Text style={styles.label}>Items en attente</Text>
            <Text style={styles.value}>{pending}</Text>
          </View>
          <TouchableOpacity
            style={styles.linkButton}
            onPress={() => navigation.navigate('SyncStatus')}
          >
            <Text style={styles.linkButtonText}>Détail sync →</Text>
          </TouchableOpacity>
        </View>

        <TouchableOpacity style={styles.logoutButton} onPress={handleLogout}>
          <Text style={styles.logoutText}>Déconnexion</Text>
        </TouchableOpacity>
      </ScrollView>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: theme.colors.background },
  content: { padding: theme.spacing.md, paddingBottom: theme.spacing.xl },
  title: {
    fontSize: theme.fontSize.xxl,
    fontWeight: '700',
    color: theme.colors.primary,
    marginBottom: theme.spacing.lg,
  },
  card: {
    backgroundColor: theme.colors.surface,
    borderRadius: theme.radius.md,
    padding: theme.spacing.md,
    marginBottom: theme.spacing.md,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  cardTitle: {
    fontSize: theme.fontSize.lg,
    fontWeight: '600',
    color: theme.colors.onBackground,
    marginBottom: theme.spacing.sm,
  },
  row: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    paddingVertical: theme.spacing.xs,
  },
  label: {
    fontSize: theme.fontSize.sm,
    color: '#757575',
    marginTop: theme.spacing.sm,
  },
  value: {
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
  },
  linkButton: { marginTop: theme.spacing.md, alignSelf: 'flex-start' },
  linkButtonText: { color: theme.colors.primary, fontWeight: '600' },
  logoutButton: {
    backgroundColor: theme.colors.error,
    paddingVertical: theme.spacing.md,
    borderRadius: theme.radius.sm,
    alignItems: 'center',
    marginTop: theme.spacing.lg,
  },
  logoutText: { color: '#ffffff', fontSize: theme.fontSize.lg, fontWeight: '600' },
});
