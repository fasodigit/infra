// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Helpers partagés pour les scénarios k6 ARMAGEDDON et KAYA.
// - Construction de JWT HS256 (dev key) sans dépendance externe.
// - Générateurs de payloads réalistes (poulets, état-civil, SOGESY).

import crypto from 'k6/crypto';
import encoding from 'k6/encoding';

// -----------------------------------------------------------------------------
// JWT HS256 — clé de développement (lue depuis env, fallback dev).
// NE JAMAIS réutiliser cette clé en production (cf. Vault faso/auth-ms/jwt).
// -----------------------------------------------------------------------------

const DEV_JWT_SECRET: string = (__ENV.FASO_JWT_DEV_KEY as string) || 'faso-dev-jwt-key-not-for-prod';

function b64url(input: string | ArrayBuffer): string {
  return encoding
    .b64encode(input, 'rawstd')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/g, '');
}

/**
 * Construit un JWT HS256 minimal signé avec la clé dev.
 * Claims par défaut : sub, iat, exp (+1h), iss=faso-dev.
 */
export function buildDevJwt(sub: string = 'load-test-user', extraClaims: Record<string, unknown> = {}): string {
  const header = { alg: 'HS256', typ: 'JWT' };
  const now = Math.floor(Date.now() / 1000);
  const payload = {
    sub,
    iss: 'faso-dev',
    aud: 'faso-services',
    iat: now,
    exp: now + 3600,
    ...extraClaims,
  };
  const headerPart = b64url(JSON.stringify(header));
  const payloadPart = b64url(JSON.stringify(payload));
  const signingInput = `${headerPart}.${payloadPart}`;
  const signature = crypto.hmac('sha256', DEV_JWT_SECRET, signingInput, 'binary');
  const sigPart = b64url(signature as unknown as ArrayBuffer);
  return `${signingInput}.${sigPart}`;
}

// -----------------------------------------------------------------------------
// Payload generators
// -----------------------------------------------------------------------------

function randInt(min: number, max: number): number {
  return Math.floor(Math.random() * (max - min + 1)) + min;
}

function pick<T>(arr: T[]): T {
  return arr[randInt(0, arr.length - 1)];
}

const RACES_POULETS = ['Cobb500', 'Ross308', 'LocaleGallus', 'Sasso', 'Hubbard'];
const VILLES_BF = ['Ouagadougou', 'Bobo-Dioulasso', 'Koudougou', 'Banfora', 'Ouahigouya', 'Kaya', 'Tenkodogo'];
const NOMS_FR = ['Ouedraogo', 'Sawadogo', 'Compaore', 'Kabore', 'Traore', 'Zongo', 'Kinda', 'Nikiema'];
const PRENOMS_FR = ['Aminata', 'Salif', 'Issouf', 'Fatimata', 'Moussa', 'Rasmata', 'Boukari', 'Awa'];

/**
 * Génère un payload "poulet" (~1 KB JSON) conforme au schéma poulets-api.
 */
export function poulet1KB(): string {
  const payload = {
    batchId: `batch-${randInt(1000, 99999)}`,
    race: pick(RACES_POULETS),
    poidsKg: (0.8 + Math.random() * 3.2).toFixed(3),
    ageJours: randInt(1, 90),
    eleveurId: `eleveur-${randInt(1, 5000)}`,
    localite: pick(VILLES_BF),
    prixCfa: randInt(2500, 7500),
    disponible: true,
    vaccinations: [
      { nom: 'Newcastle', date: '2026-03-01', lot: `LOT-${randInt(100, 999)}` },
      { nom: 'Gumboro',   date: '2026-03-15', lot: `LOT-${randInt(100, 999)}` },
      { nom: 'Marek',     date: '2026-02-20', lot: `LOT-${randInt(100, 999)}` },
    ],
    alimentation: {
      typeAliment: pick(['demarrage', 'croissance', 'finition']),
      quantiteKgParJour: (0.05 + Math.random() * 0.15).toFixed(3),
      fournisseur: pick(['SONACEB', 'FASO-PROVENDE', 'CooperativeLocale']),
    },
    traçabilite: {
      idExploitation: `EXP-${randInt(1, 2000)}`,
      region: pick(['Centre', 'Hauts-Bassins', 'Est', 'Sahel', 'Boucle Mouhoun']),
      certifieBio: Math.random() < 0.15,
      dateArrivee: '2026-04-01T08:30:00Z',
    },
    metadata: {
      source: 'load-test-k6',
      version: '1.0',
      tags: ['faso', 'poulets', 'load'],
    },
  };
  return JSON.stringify(payload);
}

/**
 * Génère un payload "demande de certificat d'état civil" (SOGESY fake).
 */
export function etatCivilCertificateRequest(): string {
  const payload = {
    typeActe: pick(['naissance', 'mariage', 'deces']),
    demandeur: {
      nom: pick(NOMS_FR),
      prenom: pick(PRENOMS_FR),
      nni: `${randInt(1000000, 9999999)}${randInt(100, 999)}`,
      dateNaissance: `19${randInt(60, 99)}-${String(randInt(1, 12)).padStart(2, '0')}-${String(randInt(1, 28)).padStart(2, '0')}`,
      lieuNaissance: pick(VILLES_BF),
    },
    communeCompetente: pick(VILLES_BF),
    motif: pick(['passeport', 'scolarite', 'mariage', 'travail', 'succession']),
    urgent: Math.random() < 0.2,
  };
  return JSON.stringify(payload);
}

/**
 * Simule une clé KAYA réaliste selon le domaine (poulets, sessions, caches).
 */
export function kayaKey(prefix: string = 'poulet'): string {
  return `${prefix}:${randInt(1, 1_000_000)}`;
}

/**
 * Valeur courte (<256 B) pour SET/GET.
 */
export function kayaSmallValue(): string {
  return JSON.stringify({ id: randInt(1, 1e9), ts: Date.now(), v: pick(VILLES_BF) });
}
