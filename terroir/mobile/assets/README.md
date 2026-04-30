<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# assets/

Placeholder pour les assets statiques de l'app TERROIR mobile.

À fournir par le designer (P0.J ou P1.0) :

| Fichier | Format | Dimension | Usage |
|---------|--------|-----------|-------|
| `icon.png` | PNG (alpha) | 1024x1024 | Icone app (Expo génère les variantes Android / iOS) |
| `splash.png` | PNG (alpha) | 2048x2048 | Splash screen (background `#1b5e20`, savane BF) |
| `adaptive-icon.png` | PNG (alpha) | 1024x1024 | Foreground adaptive Android (Android 8+) |
| `favicon.png` | PNG | 48x48 | Favicon (mode web Expo, secondaire) |

Tant que ces fichiers ne sont pas fournis, `expo start` affichera un
warning mais l'app continuera de tourner avec les fallbacks Expo.

## Localisation prévue (P1)

Logo TERROIR définitif sera fourni par l'équipe FASO design — référence
palette : vert savane `#1b5e20` + or sahel `#f9a825`.
