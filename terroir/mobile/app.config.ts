// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * TERROIR mobile — programmatic Expo config.
 *
 * Lit la variable d'environnement EAS_UPDATE_URL pour pointer vers l'instance
 * EAS Update self-hosted (souverain). Fallback : placeholder
 * https://updates.terroir.faso.bf (à remplacer par l'URL effective de
 * l'instance self-hosted déployée par P0.J / DevOps).
 *
 * TODO P1 : ne JAMAIS basculer sur Expo public cloud par défaut. Toute
 * tentative d'utiliser updates.expo.dev doit être commentée et discutée
 * en ADR (souveraineté FASO).
 */
import type { ExpoConfig, ConfigContext } from 'expo/config';

const SELF_HOSTED_FALLBACK = 'https://updates.terroir.faso.bf';

export default ({ config }: ConfigContext): ExpoConfig => {
  const easUpdateUrl = process.env.EAS_UPDATE_URL ?? SELF_HOSTED_FALLBACK;
  const apiBaseUrl =
    process.env.TERROIR_API_BASE_URL ?? 'http://10.0.2.2:8080/api/terroir/mobile-bff';

  return {
    ...(config as ExpoConfig),
    name: 'TERROIR',
    slug: 'terroir-mobile',
    updates: {
      ...(config.updates ?? {}),
      url: easUpdateUrl,
    },
    extra: {
      ...(config.extra ?? {}),
      apiBaseUrl,
      easUpdateUrl,
    },
  };
};
