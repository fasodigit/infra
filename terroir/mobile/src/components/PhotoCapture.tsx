// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * PhotoCapture — wrapper expo-image-picker (camera + galerie).
 *
 * Comportement :
 *   - 1er bouton : ouvre la caméra (`launchCameraAsync`).
 *   - 2nd bouton : ouvre la galerie (`launchImageLibraryAsync`).
 *   - URI résultat (file://) remontée via onChange.
 *   - Preview thumbnail si photo capturée.
 *
 * Pourquoi pas expo-camera direct ? L'`ImagePicker` natif gère permission
 * + UI standard du device (familiarité agent terrain). expo-camera reste
 * dispo si on a besoin d'overlay custom (ex : guide CNIB) — TODO P2.
 */
import { useState } from 'react';
import { Alert, Image, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import * as ImagePicker from 'expo-image-picker';

import { theme } from '../theme';

interface Props {
  label?: string;
  onChange?: (uri: string | null) => void;
}

export function PhotoCapture({ label = 'Photo', onChange }: Props) {
  const [uri, setUri] = useState<string | null>(null);

  async function takePhoto() {
    const perm = await ImagePicker.requestCameraPermissionsAsync();
    if (!perm.granted) {
      Alert.alert('Permission caméra refusée');
      return;
    }
    const result = await ImagePicker.launchCameraAsync({
      mediaTypes: ImagePicker.MediaTypeOptions.Images,
      quality: 0.7,
      allowsEditing: false,
      exif: false,
    });
    if (!result.canceled && result.assets.length > 0) {
      const next = result.assets[0].uri;
      setUri(next);
      onChange?.(next);
    }
  }

  async function pickPhoto() {
    const perm = await ImagePicker.requestMediaLibraryPermissionsAsync();
    if (!perm.granted) {
      Alert.alert('Permission galerie refusée');
      return;
    }
    const result = await ImagePicker.launchImageLibraryAsync({
      mediaTypes: ImagePicker.MediaTypeOptions.Images,
      quality: 0.7,
      allowsEditing: false,
    });
    if (!result.canceled && result.assets.length > 0) {
      const next = result.assets[0].uri;
      setUri(next);
      onChange?.(next);
    }
  }

  function clear() {
    setUri(null);
    onChange?.(null);
  }

  return (
    <View style={styles.container}>
      <Text style={styles.label}>{label}</Text>
      {uri !== null ? (
        <View style={styles.previewWrap}>
          <Image source={{ uri }} style={styles.preview} resizeMode="cover" />
          <TouchableOpacity style={styles.clearButton} onPress={clear}>
            <Text style={styles.clearText}>Supprimer</Text>
          </TouchableOpacity>
        </View>
      ) : null}
      <View style={styles.buttons}>
        <TouchableOpacity style={styles.button} onPress={() => void takePhoto()}>
          <Text style={styles.buttonText}>Prendre une photo</Text>
        </TouchableOpacity>
        <TouchableOpacity style={styles.buttonSecondary} onPress={() => void pickPhoto()}>
          <Text style={styles.buttonSecondaryText}>Galerie</Text>
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
  previewWrap: {
    marginBottom: theme.spacing.sm,
  },
  preview: {
    width: '100%',
    height: 200,
    borderRadius: theme.radius.sm,
    backgroundColor: '#eeeeee',
  },
  clearButton: {
    marginTop: theme.spacing.xs,
    alignSelf: 'flex-end',
  },
  clearText: {
    color: theme.colors.error,
    fontWeight: '600',
  },
  buttons: {
    flexDirection: 'row',
    gap: theme.spacing.sm,
  },
  button: {
    flex: 1,
    backgroundColor: theme.colors.primary,
    paddingVertical: theme.spacing.sm,
    borderRadius: theme.radius.sm,
    alignItems: 'center',
  },
  buttonText: {
    color: theme.colors.onPrimary,
    fontWeight: '600',
  },
  buttonSecondary: {
    flex: 1,
    backgroundColor: theme.colors.surface,
    paddingVertical: theme.spacing.sm,
    borderRadius: theme.radius.sm,
    alignItems: 'center',
    borderWidth: 1,
    borderColor: theme.colors.primary,
  },
  buttonSecondaryText: {
    color: theme.colors.primary,
    fontWeight: '600',
  },
});
