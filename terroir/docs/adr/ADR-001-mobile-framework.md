<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# ADR-001 — Framework de l'application agent terrain

| Champ | Valeur |
|---|---|
| Statut | Proposé |
| Date | 2026-04-30 |
| Décideurs | Tech lead, designer UX, BizDev |
| Contexte | TERROIR P1 — app agent collecteur, hors-ligne 14j, Android Go (1 GB RAM) |

## Contexte

L'app agent terrain est le canal critique de collecte primaire (saisie membres, livraisons, GPS). Contraintes :

1. **Hors-ligne** 7-14 jours en brousse (tournée campagne)
2. **Hardware** : Android Go / smartphones bas de gamme (1 GB RAM, stockage 8-16 GB)
3. **Réseau** : 2G/EDGE majoritaire, 3G/4G en zone urbaine
4. **Multilingue** : FR + 6 langues nationales (Mooré, Dioula, Fulfuldé, Bambara, Wolof, Hausa)
5. **Capteurs** : GPS, caméra (photos parcelle + CNIB), Bluetooth (balance), NFC (carte producteur)
6. **Pool dev** : équipe small (2 frontend dev, plus à l'aise React)
7. **Vitesse de livraison** : MVP P1 en 12 semaines

## Options envisagées

### Option A — React Native + Expo (Bare workflow)
**Pour**
- Mêmes devs que le web admin (React) — pool unique
- Expo SDK 51+ : modules natifs GPS, BLE, NFC, caméra prêts à l'emploi
- OTA updates via EAS (réduit les déploiements PlayStore en zone faible bande passante)
- Hermes engine : empreinte mémoire OK sur Android Go (mesurée ~80 MB cold start)
- TypeScript bout-en-bout (cohérent avec terroir-web-admin)

**Contre**
- Bridge JS-Natif moins performant que Flutter pour rendu lourd (non bloquant ici, formulaires + carte)
- Dépendance Expo (mais bare workflow garde porte de sortie)

### Option B — Flutter
**Pour**
- Performance UI supérieure (Skia direct, 60fps stable)
- Empreinte APK plus prévisible
- Bonne intégration BLE et caméra

**Contre**
- Dart : aucun dev FASO actuel ne le maîtrise → courbe d'apprentissage 4-6 sem
- Codebase séparé du web admin (duplication composants)
- Écosystème offline-first moins mature côté CRDT

### Option C — Native Android (Kotlin) only
**Pour**
- Performance maximale sur Android Go
- Accès direct à toutes les API plateforme

**Contre**
- Ne couvre pas iOS (futur portail tier acheteur peut-être iPad)
- Coût dev x2 si extension iOS plus tard
- Stack hétérogène (web React + mobile Kotlin)

### Option D — PWA (Progressive Web App)
**Pour**
- Aucune installation, MAJ instantanée
- Mêmes devs que web admin

**Contre**
- Permissions natives limitées sur Android (BLE limité, NFC quasi inexistant en PWA)
- Service Worker offline OK mais sync background limité
- Stockage IndexedDB + chiffrement local plus fragile

## Décision

**React Native + Expo (bare workflow), avec Hermes activé.**

## Justification

1. Pool de devs cohérent avec le web admin → vélocité 12-sem MVP atteignable
2. Expo modules couvrent toutes les API capteurs nécessaires (caméra, GPS, BLE, NFC)
3. EAS Update permet OTA en zone bande passante (delta < 2 MB typique)
4. Hermes garde empreinte mémoire compatible Android Go
5. Codebase TS partagé partiellement avec terroir-web-admin (validation Zod, types domaine)
6. Porte de sortie vers natif possible (bare workflow + react-native-modules)

iOS n'est **pas** prioritaire P1 mais reste possible sans refonte (RN cross-plateforme).

## Conséquences

### Positives
- Une seule équipe frontend
- OTA réduit pression PlayStore
- Hot-reload accélère cycles UX langues nationales

### Négatives / risques
- Dépendance forte à l'écosystème Expo (vendor lock-in modéré)
- Bridge JS-Natif : si module BLE custom requis pour balances exotiques, devra écrire un module natif
- Performance carte interactive (Mapbox GL Native) à benchmarker dès P1 sur Android Go

### Mitigations
- Bare workflow dès J0 (permet ejection sans drama)
- Feature flag carte → fallback liste si device < seuil RAM
- Bench obligatoire P1 sur Tecno Spark Go (référence Android Go en BF)

## Métriques de succès

- Cold start ≤ 3 s sur Tecno Spark Go
- Sync 50 livraisons + 20 photos compressées ≤ 2 minutes en EDGE
- APK ≤ 25 MB
- 0 crash sur 100 livraisons synthétiques

## Révision

À reconfirmer fin P1 (12 sem). Si benchmark Android Go échoue → réévaluer Flutter.
