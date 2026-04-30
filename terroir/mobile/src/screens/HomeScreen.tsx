// SPDX-License-Identifier: AGPL-3.0-or-later
import { Alert, ScrollView, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import type { NativeStackScreenProps } from '@react-navigation/native-stack';

import { logout } from '../auth/kratos-client';
import { HealthIndicator } from '../components/HealthIndicator';
import { theme } from '../theme';
import type { RootStackParamList } from '../../App';

type Props = NativeStackScreenProps<RootStackParamList, 'Home'>;

export function HomeScreen({ navigation }: Props) {
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
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <View style={styles.header}>
        <Text style={styles.greeting}>Bonjour, agent.</Text>
        <Text style={styles.subtitle}>Tableau de bord TERROIR</Text>
      </View>

      <HealthIndicator />

      <View style={styles.card}>
        <Text style={styles.cardTitle}>Synchronisation</Text>
        <Text style={styles.cardBody}>Last sync: never</Text>
        <Text style={styles.cardBody}>Pending uploads: 0</Text>
        <TouchableOpacity
          style={styles.linkButton}
          onPress={() => navigation.navigate('SyncStatus')}
        >
          <Text style={styles.linkButtonText}>Voir le détail sync</Text>
        </TouchableOpacity>
      </View>

      <View style={styles.card}>
        <Text style={styles.cardTitle}>Modules P1</Text>
        <Text style={styles.cardBody}>
          {/* TODO P1 (cf. ULTRAPLAN §6 P1.6) : 6 écrans. */}
          - Liste producteurs (offline){'\n'}
          - Création producteur (CNIB + GPS + photo){'\n'}
          - Liste parcelles (carte MapLibre){'\n'}
          - Création parcelle (polygone GPS){'\n'}
          - Profil agent (config sync, online status)
        </Text>
      </View>

      <TouchableOpacity style={styles.logoutButton} onPress={handleLogout}>
        <Text style={styles.logoutText}>Déconnexion</Text>
      </TouchableOpacity>
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
    paddingBottom: theme.spacing.xl,
  },
  header: {
    marginBottom: theme.spacing.lg,
  },
  greeting: {
    fontSize: theme.fontSize.xxl,
    fontWeight: '700',
    color: theme.colors.primary,
  },
  subtitle: {
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
    marginTop: theme.spacing.xs,
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
  cardBody: {
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
    lineHeight: 22,
  },
  linkButton: {
    marginTop: theme.spacing.md,
    alignSelf: 'flex-start',
  },
  linkButtonText: {
    color: theme.colors.primary,
    fontSize: theme.fontSize.md,
    fontWeight: '600',
  },
  logoutButton: {
    backgroundColor: theme.colors.error,
    paddingVertical: theme.spacing.md,
    borderRadius: theme.radius.sm,
    alignItems: 'center',
    marginTop: theme.spacing.lg,
  },
  logoutText: {
    color: '#ffffff',
    fontSize: theme.fontSize.lg,
    fontWeight: '600',
  },
});
