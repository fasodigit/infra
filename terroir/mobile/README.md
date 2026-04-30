<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# terroir-mobile

Application **agent terrain** TERROIR — React Native + Expo SDK 53,
TypeScript strict, offline-first, CRDT Yjs.

> **Statut P0.G** : scaffold uniquement (aucune logique métier P1).
> Dépendances `node_modules/` non installées : à faire par P0.J ou par
> l'utilisateur via `bun install`.

---

## Stack

| Composant | Choix | Référence |
|-----------|-------|-----------|
| Framework | React Native + Expo SDK 53 (Hermes) | ADR-001 |
| Storage local secrets | `expo-secure-store` (Android Keystore / iOS Keychain) | — |
| Storage local data | `expo-sqlite` (WAL mode) | — |
| CRDT | `yjs` + adapter SQLite custom | ADR-002 + décision User P0.G |
| Navigation | `@react-navigation/native-stack` v7 | — |
| OTA | EAS Update **self-hosted** (souverain) | Décision User P0.G |
| Build | EAS Build (APK / AAB) | `eas.json` |
| Submission Play | Track `internal` → équipe FASO | `eas.json` |
| Cible OS | Android API 24+ (Android 7+, ex Tecno Spark Go) | ULTRAPLAN §4 P0.8 |
| APK target | ≤ 25 MB | ULTRAPLAN §1 |

## Pré-requis

- **Bun** ≥ 1.1 ou **Node** 20+ (les deux fonctionnent avec Expo SDK 53).
- **Android Studio** + émulateur API 24+ pour `bun android`.
- (Optionnel iOS) Xcode 15+ macOS pour `bun ios`.
- `eas-cli` global :
  ```bash
  bun install -g eas-cli
  ```
- Accès Vault FASO pour récupérer le keystore Android :
  ```bash
  vault kv get -field=apk-keystore-jks faso/terroir/apk-keystore > /tmp/terroir.jks
  vault kv get -field=apk-keystore-password faso/terroir/apk-keystore
  ```

## Installation locale

```bash
cd INFRA/terroir/mobile
bun install
bun start         # Expo Go / dev server (Metro)
bun android       # Emulateur Android (10.0.2.2 = host loopback)
```

L'app pointe par défaut sur :

```
http://10.0.2.2:8080/api/terroir/mobile-bff
```

(ARMAGEDDON :8080 sur le host → cluster `terroir-mobile-bff` :8833.)

Override en local :

```bash
TERROIR_API_BASE_URL="http://192.168.1.42:8080/api/terroir/mobile-bff" bun start
```

## EAS self-hosted (souveraineté)

L'instance Expo public cloud n'est **pas** utilisée. Toute publication
OTA passe par notre EAS Update self-hosted.

Variable obligatoire avant `eas update` :

```bash
export EAS_UPDATE_URL="https://updates.terroir.faso.bf"   # à remplacer par l'URL effective
eas update --branch production --message "P1.x — release notes"
```

Le placeholder `https://updates.terroir.faso.bf` est défini dans
`app.config.ts` ; ne pas committer d'URL `updates.expo.dev` — c'est une
violation souveraineté FASO (cf. `INFRA/CLAUDE.md` §3).

### Secrets EAS (Vault)

| Path Vault | Champ | Usage |
|-------------|-------|-------|
| `faso/terroir/apk-keystore` | `apk-keystore-jks` (base64) | Signing APK Android |
| `faso/terroir/apk-keystore` | `apk-keystore-password` | Mot de passe keystore |
| `faso/terroir/playstore` | `service-account-json` | Submission `eas submit` |
| `faso/terroir/eas` | `update-private-key` | Signature manifestes EAS Update self-hosted |

EAS attend ces secrets montés sur `/vault-secrets/` (Vault Agent
sidecar dans le container `eas` du worker self-hosted, déployé en P0.J).

## Structure

```
mobile/
├── app.json              # Expo config statique
├── app.config.ts         # Override env-driven (EAS_UPDATE_URL, API base)
├── eas.json              # Build + Update + Submit config
├── package.json          # deps Expo SDK 53
├── tsconfig.json         # TS strict + path alias @/*
├── babel.config.js       # preset-expo + module-resolver
├── metro.config.js       # default Expo
├── App.tsx               # Stack navigator (Login / Home / SyncStatus)
├── src/
│   ├── api/              # fetch wrapper + types BFF
│   ├── auth/             # JWT SecureStore + Kratos client
│   ├── crdt/             # Yjs store + SQLite adapter + parcel doc
│   ├── screens/          # 3 écrans P0 (placeholder)
│   ├── components/       # HealthIndicator
│   ├── i18n/             # FR + EN (Mooré / Dioula / Fulfuldé etc. P1)
│   └── theme.ts
└── assets/               # Icons / splash (à fournir par design)
```

## Limitations P0.G

- Pas de logique métier (les écrans sont des placeholders UI).
- Pas de tests RN (Detox / Maestro en P1+, hors-scope P0).
- `node_modules/` non installées (faire `bun install` après merge).
- `assets/icon.png`, `assets/splash.png` non fournis (placeholders dans
  `assets/README.md`).
- Multilingue : seulement FR + EN. Mooré / Dioula / Fulfuldé / Bambara /
  Wolof / Hausa en P1.6.
- Pas de carte (MapLibre P1.6).

## Plan P1.6 — 6 écrans MVP

Référence : `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §6 P1.6.

1. **Login** (Kratos JWT, MFA optionnelle).
2. **Profil agent terrain** (config sync, online status, langue).
3. **Liste producteurs** (recherche, filtres, offline SQLite).
4. **Création producteur** (CNIB capture, GPS, photo).
5. **Liste parcelles** (carte MapLibre, polygones depuis Yjs).
6. **Création parcelle** (polygone GPS, marche/pas-à-pas).

Modules natifs additionnels P1 : `expo-location`, `expo-camera`,
`expo-image-picker`, `expo-barcode-scanner` (CNIB QR), `react-native-ble-plx`
(balance Bluetooth), `@maplibre/maplibre-react-native`.

## Backend

L'app consomme `terroir-mobile-bff` (Rust, :8833, P1.5) via ARMAGEDDON
:8080 — cf. ULTRAPLAN §4 P0.8 routing. Endpoints documentés OpenAPI
(générés par tower-spectre P1).

## Sécurité

- JWT stocké dans `expo-secure-store` (Android Keystore HW-backed).
- DEK Vault Transit ciphertext stocké dans SecureStore (jamais déchiffré
  côté device — chiffrement au-dessus de SQLite implémenté en P1, cf.
  ADR-005).
- TLS only en prod (HTTP autorisé uniquement vers `10.0.2.2:8080` dev
  loopback).
- Pas de log des secrets (interdiction `console.log(jwt)` — règle ESLint
  P1).
- AGPL-3.0-or-later : header SPDX sur chaque source.

## ADR à lire

- `INFRA/terroir/docs/adr/ADR-001-mobile-framework.md`
- `INFRA/terroir/docs/adr/ADR-002-sync-conflict-resolution.md`
- `INFRA/terroir/docs/adr/ADR-005-pii-encryption.md`

## Référence ULTRAPLAN

`INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` :
- §1 ADR-001
- §4 P0.8 (bootstrap = ce stream P0.G)
- §6 P1.6 (modules MVP)
