// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Création producteur — formulaire avec :
 *   - full_name, NIN (CNIB), phone (validation simple)
 *   - GPS domicile (auto via expo-location)
 *   - photo (camera / galerie)
 *
 * Submit : enfile un SyncItem `producer-update` (lwwVersion=0 = create
 * côté backend) dans `sync-queue` SQLite. Le worker le push au prochain
 * cycle 60s vers `/m/sync/batch`.
 *
 * Mode offline : tout est stocké localement, l'agent peut continuer de
 * créer des fiches sans réseau ; sync au retour de connectivité.
 */
import { useState } from 'react';
import {
  Alert,
  KeyboardAvoidingView,
  Platform,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  TouchableOpacity,
  View,
} from 'react-native';
import type { NativeStackScreenProps } from '@react-navigation/native-stack';

import { enqueueSyncItem } from '../api/sync-queue';
import { GpsField, type GpsCoords } from '../components/GpsField';
import { PhotoCapture } from '../components/PhotoCapture';
import { SyncStatusBanner } from '../components/SyncStatusBanner';
import { theme } from '../theme';
import type { RootStackParamList } from '../../App';

type Props = NativeStackScreenProps<RootStackParamList, 'ProducerCreate'>;

const NIN_REGEX = /^B[A-Z0-9]{10,14}$/i; // CNIB Burkina — pattern indicatif
const PHONE_REGEX = /^[+]?[0-9 ]{8,16}$/;

// UUID v4 light (cohérent avec sync-queue)
function uuidv4(): string {
  const hex = (n: number) => n.toString(16).padStart(2, '0');
  const bytes = new Uint8Array(16);
  for (let i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const b = Array.from(bytes, hex);
  return `${b.slice(0, 4).join('')}-${b.slice(4, 6).join('')}-${b.slice(6, 8).join('')}-${b
    .slice(8, 10)
    .join('')}-${b.slice(10, 16).join('')}`;
}

export function ProducerCreateScreen({ navigation }: Props) {
  const [fullName, setFullName] = useState('');
  const [nin, setNin] = useState('');
  const [phone, setPhone] = useState('');
  const [primaryCrop, setPrimaryCrop] = useState('');
  const [coords, setCoords] = useState<GpsCoords | null>(null);
  const [photoUri, setPhotoUri] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  function validate(): string | null {
    if (fullName.trim().length < 3) return 'Nom complet trop court.';
    if (nin.trim().length > 0 && !NIN_REGEX.test(nin.trim())) {
      return 'Format CNIB invalide (ex : B12345678901).';
    }
    if (phone.trim().length > 0 && !PHONE_REGEX.test(phone.trim())) {
      return 'Format téléphone invalide.';
    }
    if (coords === null) return 'Position GPS du domicile requise.';
    return null;
  }

  async function onSubmit() {
    const err = validate();
    if (err !== null) {
      Alert.alert('Validation', err);
      return;
    }
    setSubmitting(true);
    try {
      const producerId = uuidv4();
      await enqueueSyncItem({
        type: 'producer-update',
        producerId,
        lwwVersion: 0,
        patch: {
          full_name: fullName.trim(),
          nin: nin.trim() || null,
          phone: phone.trim() || null,
          primary_crop: primaryCrop.trim() || null,
          home_lat: coords?.latitude,
          home_lng: coords?.longitude,
          home_accuracy_m: coords?.accuracy,
          // TODO P2 : upload photo via /m/uploads (multipart) puis stocker URI
          //          et ref dans patch ; pour P1, conservée en local seulement.
          photo_local_uri: photoUri,
        },
      });
      Alert.alert(
        'Créé',
        'Producteur enregistré localement. Sync au prochain cycle réseau.',
        [{ text: 'OK', onPress: () => navigation.goBack() }],
      );
    } catch (e) {
      Alert.alert('Erreur', e instanceof Error ? e.message : 'Échec enregistrement.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <View style={styles.container}>
      <SyncStatusBanner />
      <KeyboardAvoidingView
        style={{ flex: 1 }}
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      >
        <ScrollView contentContainerStyle={styles.content}>
          <Text style={styles.title}>Nouveau producteur</Text>

          <Text style={styles.label}>Nom complet *</Text>
          <TextInput
            style={styles.input}
            value={fullName}
            onChangeText={setFullName}
            placeholder="Ex : Aminata Ouédraogo"
            placeholderTextColor="#9e9e9e"
          />

          <Text style={styles.label}>CNIB (NIN)</Text>
          <TextInput
            style={styles.input}
            value={nin}
            onChangeText={setNin}
            autoCapitalize="characters"
            placeholder="B12345678901"
            placeholderTextColor="#9e9e9e"
          />

          <Text style={styles.label}>Téléphone</Text>
          <TextInput
            style={styles.input}
            value={phone}
            onChangeText={setPhone}
            keyboardType="phone-pad"
            placeholder="+226 70 00 00 00"
            placeholderTextColor="#9e9e9e"
          />

          <Text style={styles.label}>Culture principale</Text>
          <TextInput
            style={styles.input}
            value={primaryCrop}
            onChangeText={setPrimaryCrop}
            placeholder="coton / sésame / karité / anacarde"
            placeholderTextColor="#9e9e9e"
          />

          <GpsField
            label="GPS domicile *"
            autoFetch
            onChange={setCoords}
          />

          <PhotoCapture label="Photo (optionnel)" onChange={setPhotoUri} />

          <TouchableOpacity
            style={[styles.submit, submitting && styles.submitDisabled]}
            onPress={() => void onSubmit()}
            disabled={submitting}
          >
            <Text style={styles.submitText}>
              {submitting ? 'Enregistrement…' : 'Enregistrer'}
            </Text>
          </TouchableOpacity>
        </ScrollView>
      </KeyboardAvoidingView>
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
    backgroundColor: theme.colors.surface,
  },
  submit: {
    backgroundColor: theme.colors.primary,
    paddingVertical: theme.spacing.md,
    borderRadius: theme.radius.sm,
    alignItems: 'center',
    marginTop: theme.spacing.md,
  },
  submitDisabled: { opacity: 0.6 },
  submitText: {
    color: theme.colors.onPrimary,
    fontSize: theme.fontSize.lg,
    fontWeight: '600',
  },
});
