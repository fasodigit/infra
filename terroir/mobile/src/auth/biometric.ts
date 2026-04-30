// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Stub biometric login — `expo-local-authentication`.
 *
 * P5+ : permettre `LoginScreen` à proposer "Déverrouiller avec empreinte"
 * une fois l'utilisateur déjà authentifié 1× via password (le JWT est
 * persisté dans expo-secure-store et ré-armé après scan biométrique).
 *
 * Ce module n'est PAS encore branché dans `App.tsx` ni `LoginScreen.tsx` ;
 * il fournit les signatures stables pour P5.
 */
import * as LocalAuthentication from 'expo-local-authentication';

export interface BiometricSupport {
  hardwareAvailable: boolean;
  enrolled: boolean;
  /** Types: 1=Fingerprint, 2=FaceID, 3=Iris (cf. enums expo). */
  supportedTypes: LocalAuthentication.AuthenticationType[];
}

export async function checkBiometricSupport(): Promise<BiometricSupport> {
  const [hw, enrolled, types] = await Promise.all([
    LocalAuthentication.hasHardwareAsync(),
    LocalAuthentication.isEnrolledAsync(),
    LocalAuthentication.supportedAuthenticationTypesAsync(),
  ]);
  return { hardwareAvailable: hw, enrolled, supportedTypes: types };
}

export interface BiometricPromptResult {
  success: boolean;
  reason?: string;
}

/**
 * Affiche le prompt biométrique du device. Retourne `success=true` si le
 * scan est validé. À utiliser uniquement après un login password initial.
 *
 * TODO P5+ : invalider la "biometric session" si la dernière auth password
 * date de > 30 jours (politique sécurité Q2 sliding 14j).
 */
export async function promptBiometric(reason: string): Promise<BiometricPromptResult> {
  const result = await LocalAuthentication.authenticateAsync({
    promptMessage: reason,
    cancelLabel: 'Annuler',
    fallbackLabel: 'Code PIN device',
    disableDeviceFallback: false,
  });
  if (result.success) return { success: true };
  return { success: false, reason: result.error ?? 'unknown' };
}
