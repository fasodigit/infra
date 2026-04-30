// SPDX-License-Identifier: AGPL-3.0-or-later
import { useState } from 'react';
import {
  ActivityIndicator,
  Alert,
  KeyboardAvoidingView,
  Platform,
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from 'react-native';
import type { NativeStackScreenProps } from '@react-navigation/native-stack';

import { attemptLogin, submitMfaChallenge, type MfaMethod } from '../auth/kratos-flow';
import { startSyncWorker } from '../api/sync-queue';
import { theme } from '../theme';
import type { RootStackParamList } from '../../App';

type Props = NativeStackScreenProps<RootStackParamList, 'Login'>;

type MfaState = {
  flowId: string;
  methods: MfaMethod[];
};

export function LoginScreen({ navigation }: Props) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [totp, setTotp] = useState('');
  const [mfa, setMfa] = useState<MfaState | null>(null);
  const [submitting, setSubmitting] = useState(false);

  function onSuccess() {
    // Démarre le worker offline-sync dès que la session est établie.
    startSyncWorker();
    navigation.replace('Main');
  }

  async function handleSubmit() {
    if (email.length === 0 || password.length === 0) {
      Alert.alert('Champs requis', 'Email et mot de passe sont obligatoires.');
      return;
    }
    setSubmitting(true);
    try {
      const result = await attemptLogin(email, password);
      if (result.ok) {
        onSuccess();
        return;
      }
      if (result.mfaRequired) {
        setMfa({ flowId: result.flowId, methods: result.methods });
        return;
      }
      Alert.alert('Connexion échouée', result.reason);
    } catch (err) {
      Alert.alert('Connexion échouée', err instanceof Error ? err.message : 'Erreur inconnue');
    } finally {
      setSubmitting(false);
    }
  }

  async function handleMfaSubmit() {
    if (mfa === null) return;
    if (totp.length === 0) {
      Alert.alert('Champ requis', 'Code TOTP requis.');
      return;
    }
    setSubmitting(true);
    try {
      await submitMfaChallenge({
        flowId: mfa.flowId,
        method: 'totp',
        code: totp,
      });
      onSuccess();
    } catch (err) {
      Alert.alert('Vérification échouée', err instanceof Error ? err.message : 'Erreur');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <KeyboardAvoidingView
      style={styles.container}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
    >
      <View style={styles.formCard}>
        <Text style={styles.title}>TERROIR Agent</Text>
        <Text style={styles.subtitle}>
          {mfa === null
            ? 'Connectez-vous avec vos identifiants Kratos.'
            : 'Validez le second facteur (TOTP).'}
        </Text>

        {mfa === null ? (
          <>
            <Text style={styles.label}>Email</Text>
            <TextInput
              style={styles.input}
              value={email}
              onChangeText={setEmail}
              autoCapitalize="none"
              autoCorrect={false}
              keyboardType="email-address"
              placeholder="agent@union.faso.bf"
              placeholderTextColor="#9e9e9e"
              editable={!submitting}
            />

            <Text style={styles.label}>Mot de passe</Text>
            <TextInput
              style={styles.input}
              value={password}
              onChangeText={setPassword}
              secureTextEntry
              placeholder="••••••••"
              placeholderTextColor="#9e9e9e"
              editable={!submitting}
            />

            <TouchableOpacity
              style={[styles.button, submitting && styles.buttonDisabled]}
              onPress={() => void handleSubmit()}
              disabled={submitting}
            >
              {submitting ? (
                <ActivityIndicator color={theme.colors.onPrimary} />
              ) : (
                <Text style={styles.buttonText}>Se connecter</Text>
              )}
            </TouchableOpacity>
          </>
        ) : (
          <>
            <Text style={styles.label}>Code TOTP</Text>
            <TextInput
              style={styles.input}
              value={totp}
              onChangeText={setTotp}
              keyboardType="number-pad"
              placeholder="123456"
              placeholderTextColor="#9e9e9e"
              editable={!submitting}
            />
            <TouchableOpacity
              style={[styles.button, submitting && styles.buttonDisabled]}
              onPress={() => void handleMfaSubmit()}
              disabled={submitting}
            >
              {submitting ? (
                <ActivityIndicator color={theme.colors.onPrimary} />
              ) : (
                <Text style={styles.buttonText}>Valider le code</Text>
              )}
            </TouchableOpacity>
            <TouchableOpacity onPress={() => setMfa(null)} disabled={submitting}>
              <Text style={styles.cancelLink}>← Retour à l&apos;email</Text>
            </TouchableOpacity>
          </>
        )}

        <Text style={styles.footer}>v0.1.0 — TERROIR mobile (Burkina Faso)</Text>
      </View>
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: theme.colors.background,
    justifyContent: 'center',
    paddingHorizontal: theme.spacing.lg,
  },
  formCard: {
    backgroundColor: theme.colors.surface,
    padding: theme.spacing.lg,
    borderRadius: theme.radius.md,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  title: {
    fontSize: theme.fontSize.xxl,
    fontWeight: '700',
    color: theme.colors.primary,
    marginBottom: theme.spacing.xs,
  },
  subtitle: {
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
    marginBottom: theme.spacing.lg,
  },
  label: {
    fontSize: theme.fontSize.md,
    fontWeight: '600',
    marginBottom: theme.spacing.xs,
    color: theme.colors.onBackground,
  },
  input: {
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.radius.sm,
    paddingHorizontal: theme.spacing.md,
    paddingVertical: theme.spacing.sm,
    marginBottom: theme.spacing.md,
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
    backgroundColor: theme.colors.background,
  },
  button: {
    backgroundColor: theme.colors.primary,
    paddingVertical: theme.spacing.md,
    borderRadius: theme.radius.sm,
    alignItems: 'center',
    marginTop: theme.spacing.sm,
  },
  buttonDisabled: { opacity: 0.6 },
  buttonText: {
    color: theme.colors.onPrimary,
    fontSize: theme.fontSize.lg,
    fontWeight: '600',
  },
  cancelLink: {
    color: theme.colors.primary,
    textAlign: 'center',
    marginTop: theme.spacing.md,
    fontSize: theme.fontSize.md,
  },
  footer: {
    marginTop: theme.spacing.lg,
    textAlign: 'center',
    fontSize: theme.fontSize.sm,
    color: '#757575',
  },
});
