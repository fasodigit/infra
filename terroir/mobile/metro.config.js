// SPDX-License-Identifier: AGPL-3.0-or-later
// Metro config par défaut Expo SDK 53. Override possible si modules natifs
// custom ajoutés en P1 (ex : balance BLE).
const { getDefaultConfig } = require('expo/metro-config');

const config = getDefaultConfig(__dirname);

module.exports = config;
