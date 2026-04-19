// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Synthetic-monitoring user fixture.
// Credentials are provided by Vault at runtime:
//   vault kv get faso/synthetic-monitoring/user
// and injected as SYNTHETIC_USER_EMAIL / SYNTHETIC_USER_PASSWORD env vars
// via Vault Agent (sidecar) or the cron-job ServiceAccount AppRole.
//
// The account synthetic-monitor@faso.gov.bf is a DEDICATED isolated identity
// — never a real citizen account. Data created by this user is purged nightly
// by the admin cleanup cron.

export interface SyntheticUser {
  email: string;
  password: string;
  displayName: string;
}

export function syntheticUser(): SyntheticUser {
  const email = process.env.SYNTHETIC_USER_EMAIL;
  const password = process.env.SYNTHETIC_USER_PASSWORD;

  if (!email || !password) {
    throw new Error(
      'SYNTHETIC_USER_EMAIL / SYNTHETIC_USER_PASSWORD missing. ' +
        'Fetch from Vault: vault kv get faso/synthetic-monitoring/user',
    );
  }

  if (!email.endsWith('@faso.gov.bf')) {
    throw new Error(
      `Refusing to run synthetic monitoring with non-dedicated account: ${email}`,
    );
  }

  return {
    email,
    password,
    displayName: 'Synthetic Monitor',
  };
}

export const FLOWS = {
  AUTH: 'auth',
  POULETS_ORDER: 'poulets_order',
  ETAT_CIVIL: 'etat_civil',
} as const;

export type FlowName = (typeof FLOWS)[keyof typeof FLOWS];
