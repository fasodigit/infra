// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Création parcelle — tracé polygone via PolygonDrawer (MapLibre tap-to-add)
 * + saisie crop_type + planted_at + photos.
 *
 * Persistance locale :
 *   - Yjs Doc dédié `parcel:<uuid>:polygon` avec sommets en Y.Array<Y.Map>.
 *   - À chaque tap, on append le vertex au doc (CRDT).
 *
 * Sync :
 *   - Snapshot Yjs (Y.encodeStateAsUpdate) → base64 → SyncItem
 *     `parcel-polygon-update` enfilé dans sync_queue.
 *   - LWW patch `parcel-update` (lwwVersion=0) pour crop_type / planted_at.
 *   - Si WebSocket connecté pour le producerId → diffuse le delta directement
 *     en plus de l'enqueue (fallback durable).
 */
import { useEffect, useMemo, useState } from 'react';
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
import * as Y from 'yjs';

import { enqueueSyncItem } from '../api/sync-queue';
import { PolygonDrawer } from '../components/PolygonDrawer';
import { PhotoCapture } from '../components/PhotoCapture';
import { SyncStatusBanner } from '../components/SyncStatusBanner';
import {
  appendVertex,
  encodeDocAsB64,
  loadParcelPolygonDoc,
  readVertices,
  verticesToWkt,
  type Vertex,
} from '../crdt/parcel-polygon-doc';
import { theme } from '../theme';
import type { RootStackParamList } from '../../App';

type Props = NativeStackScreenProps<RootStackParamList, 'ParcelCreate'>;

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

export function ParcelCreateScreen({ navigation, route }: Props) {
  const producerId = route.params?.producerId ?? '';
  const parcelId = useMemo(uuidv4, []);
  const [doc, setDoc] = useState<Y.Doc | null>(null);
  const [vertices, setVertices] = useState<Vertex[]>([]);
  const [cropType, setCropType] = useState('');
  const [plantedAt, setPlantedAt] = useState('');
  const [photoUri, setPhotoUri] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    void (async () => {
      const d = await loadParcelPolygonDoc(parcelId);
      setDoc(d);
    })();
  }, [parcelId]);

  function handleVerticesChange(next: Vertex[]) {
    setVertices(next);
    if (doc === null) return;
    // Diff naïf : si la longueur a augmenté de 1, append le dernier vertex.
    const current = readVertices(doc);
    if (next.length === current.length + 1) {
      appendVertex(doc, next[next.length - 1]);
    } else {
      // Sinon (clear / undo), on rejoue le tableau complet.
      doc.transact(() => {
        const arr = doc.getArray<Y.Map<unknown>>('geometry');
        arr.delete(0, arr.length);
        for (const v of next) {
          const m = new Y.Map<unknown>();
          m.set('lat', v.lat);
          m.set('lng', v.lng);
          if (v.accuracy !== undefined) m.set('accuracy', v.accuracy);
          arr.push([m]);
        }
      });
    }
  }

  async function onSubmit() {
    if (vertices.length < 3) {
      Alert.alert('Tracé incomplet', 'Au moins 3 sommets sont requis pour un polygone.');
      return;
    }
    if (cropType.trim().length === 0) {
      Alert.alert('Validation', 'Type de culture requis.');
      return;
    }
    if (doc === null) {
      Alert.alert('Erreur', 'Document Yjs non initialisé.');
      return;
    }
    setSubmitting(true);
    try {
      const yjsDelta = encodeDocAsB64(doc);
      const wkt = verticesToWkt(vertices);

      // 1) Polygon delta (CRDT — Yjs)
      await enqueueSyncItem({
        type: 'parcel-polygon-update',
        parcelId,
        yjsDelta,
      });

      // 2) Patch LWW pour métadonnées (crop_type, planted_at, surface, photo)
      await enqueueSyncItem({
        type: 'parcel-update',
        parcelId,
        lwwVersion: 0,
        patch: {
          producer_id: producerId,
          crop_type: cropType.trim(),
          planted_at: plantedAt.trim() || null,
          // surface_hectares calculé côté backend via PostGIS ; en attendant,
          // approximation grossière non envoyée.
          geom_wkt: wkt,
          photo_local_uri: photoUri,
        },
      });

      Alert.alert('Parcelle enregistrée', 'Sync au prochain cycle réseau.', [
        { text: 'OK', onPress: () => navigation.goBack() },
      ]);
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
          <Text style={styles.title}>Tracer une parcelle</Text>
          <Text style={styles.subtitle}>
            Tapez sur la carte pour ajouter un sommet. Au moins 3 sommets requis.
          </Text>

          <View style={styles.mapBlock}>
            <PolygonDrawer onChange={handleVerticesChange} />
          </View>

          <Text style={styles.label}>Type de culture *</Text>
          <TextInput
            style={styles.input}
            value={cropType}
            onChangeText={setCropType}
            placeholder="coton / sésame / karité / anacarde"
            placeholderTextColor="#9e9e9e"
          />

          <Text style={styles.label}>Date de plantation (AAAA-MM-JJ)</Text>
          <TextInput
            style={styles.input}
            value={plantedAt}
            onChangeText={setPlantedAt}
            placeholder="2026-04-30"
            placeholderTextColor="#9e9e9e"
          />

          <PhotoCapture label="Photo parcelle (optionnel)" onChange={setPhotoUri} />

          <TouchableOpacity
            style={[styles.submit, submitting && styles.submitDisabled]}
            onPress={() => void onSubmit()}
            disabled={submitting}
          >
            <Text style={styles.submitText}>
              {submitting ? 'Enregistrement…' : 'Enregistrer la parcelle'}
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
    marginBottom: theme.spacing.xs,
  },
  subtitle: {
    fontSize: theme.fontSize.md,
    color: theme.colors.onBackground,
    marginBottom: theme.spacing.md,
  },
  mapBlock: {
    height: 360,
    marginBottom: theme.spacing.md,
    borderRadius: theme.radius.md,
    overflow: 'hidden',
    borderWidth: 1,
    borderColor: theme.colors.border,
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
