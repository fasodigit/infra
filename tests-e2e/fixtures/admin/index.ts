// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Admin fixtures barrel export — exposé par les 33 specs Phase 4.d
 * (cf. ARCHITECTURE-SECURITE-COMPLETE-2026-04-30.md §5).
 */
export { AdminApiClient } from './admin-api-client';
export type { AdminApiResponse, KratosLoginResult } from './admin-api-client';
export { MagicLinkHelper } from './magic-link-helper';
export type { ExtractedMagicLink } from './magic-link-helper';
export { PushApprovalHelper } from './push-approval-helper';
export type { PushWsResult } from './push-approval-helper';
export { RiskHelper } from './risk-helper';
export { PgAdminProbe } from './pg-admin-probe';
export type { PgProbeResult } from './pg-admin-probe';

import { AdminApiClient } from './admin-api-client';
import { MailpitClient } from '../mailpit';
import { seededSuperAdmins, type SeededAdmin } from '../actors';

export interface AdminSuiteContext {
  admin: AdminApiClient;
  mailpit: MailpitClient;
  aminata: SeededAdmin;
  souleymane: SeededAdmin;
  reachability: { gateway: boolean; bff: boolean; kratos: boolean; mailpit: boolean };
  loginOk: boolean;
}

/**
 * Bootstrap utilisé par chaque spec admin :
 *   - vérifie la santé de la stack
 *   - login Aminata via Kratos
 *   - retourne le contexte ; si gate KO, la spec doit
 *     `testInfo.skip(true, reason)`.
 */
export async function bootstrapAdminSuite(): Promise<AdminSuiteContext> {
  const admin = new AdminApiClient();
  const mailpit = new MailpitClient();
  const reachability = {
    ...(await admin.isReachable()),
    mailpit: await mailpit.isReachable(),
  };
  let loginOk = false;
  if (reachability.kratos) {
    const aminata = seededSuperAdmins[0];
    if (aminata) {
      const r = await admin.login(aminata.email, aminata.password);
      loginOk = r.ok;
    }
  }
  return {
    admin,
    mailpit,
    aminata: seededSuperAdmins[0]!,
    souleymane: seededSuperAdmins[1]!,
    reachability,
    loginOk,
  };
}
