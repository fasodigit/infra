// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * Schemas Zod pour les bodies POST/PUT des routes admin.
 *
 * Tous strict (no `.passthrough()`) afin que le BFF rejette tout payload
 * inattendu — la défense en profondeur exige de valider à chaque saut.
 *
 * Référence : `INFRA/docs/GAP-ANALYSIS-PHASE-4A.md` §3 + contrat extrait
 * des services frontend `frontend/src/app/features/admin/services/*`.
 */

import { z } from 'zod';

// ---------------------------------------------------------------------------
// Constantes & helpers communs
// ---------------------------------------------------------------------------

export const AdminLevelSchema = z.enum(['SUPER-ADMIN', 'ADMIN', 'MANAGER']);
export type AdminLevel = z.infer<typeof AdminLevelSchema>;

export const ScopeSchema = z.enum(['TOUS', 'DIRECTION']);

const Iso8601 = z
  .string()
  .min(10)
  .max(40)
  .regex(/^\d{4}-\d{2}-\d{2}/, 'must be ISO-8601 date or datetime');

const NonEmpty = (max = 4096) => z.string().min(1).max(max);

const Uuid = z.string().uuid();

const Email = z.string().email().max(254);

const OtpId = z.string().min(8).max(128);

// 6, 8 chiffres ou format alphanumérique 4-12 chars (recovery)
const OtpCode = z.string().min(4).max(20).regex(/^[A-Z0-9-]+$/i);

// ---------------------------------------------------------------------------
// 1. Users / Roles
// ---------------------------------------------------------------------------

export const InviteAdminSchema = z
  .object({
    email: Email,
    role: AdminLevelSchema,
    department: NonEmpty(120).optional(),
    locale: z.enum(['fr', 'en']).default('fr').optional(),
  })
  .strict();
export type InviteAdminRequest = z.infer<typeof InviteAdminSchema>;

export const GrantRoleRequestSchema = z
  .object({
    targetRole: AdminLevelSchema,
    justification: NonEmpty(2000),
    otpCode: OtpCode,
    scope: ScopeSchema.optional(),
    tenantId: NonEmpty(120).optional(),
    expiresAt: Iso8601.optional(),
    // delta 2026-04-30 §3.4 — capabilities granulaires (registry)
    capabilities: z.array(z.string().min(1).max(120)).min(1).max(64),
  })
  .strict();
export type GrantRoleRequest = z.infer<typeof GrantRoleRequestSchema>;

export const RevokeRoleRequestSchema = z
  .object({
    targetRole: AdminLevelSchema,
    justification: NonEmpty(2000),
    otpCode: OtpCode,
  })
  .strict();
export type RevokeRoleRequest = z.infer<typeof RevokeRoleRequestSchema>;

export const SuspendUserRequestSchema = z
  .object({
    motif: NonEmpty(2000),
    until: Iso8601.optional(),
  })
  .strict();
export type SuspendUserRequest = z.infer<typeof SuspendUserRequestSchema>;

export const MfaResetRequestSchema = z
  .object({
    motif: NonEmpty(2000),
    methods: z.array(z.enum(['totp', 'passkey', 'recovery'])).min(1),
    otpCode: OtpCode,
  })
  .strict();
export type MfaResetRequest = z.infer<typeof MfaResetRequestSchema>;

// ---------------------------------------------------------------------------
// 2. OTP / Settings / Audit
// ---------------------------------------------------------------------------

export const OtpIssueSchema = z
  .object({
    userId: Uuid.optional(),
    method: z.enum(['email', 'sms', 'totp']),
    purpose: z
      .enum([
        'admin-login',
        'grant-role',
        'revoke-role',
        'break-glass',
        'settings-update',
        'mfa-reset',
        'recovery-codes',
      ])
      .optional(),
  })
  .strict();
export type OtpIssueRequest = z.infer<typeof OtpIssueSchema>;

export const OtpVerifySchema = z
  .object({
    otpId: OtpId,
    code: OtpCode,
  })
  .strict();
export type OtpVerifyRequest = z.infer<typeof OtpVerifySchema>;

export const SettingValueSchema = z.union([
  z.string().max(8192),
  z.number().finite(),
  z.boolean(),
  z.array(z.union([z.string(), z.number(), z.boolean()])).max(256),
  z.record(z.string(), z.union([z.string(), z.number(), z.boolean()])),
]);

export const SettingUpdateSchema = z
  .object({
    value: SettingValueSchema,
    version: z.number().int().nonnegative(),
    motif: NonEmpty(2000).optional(),
  })
  .strict();
export type SettingUpdateRequest = z.infer<typeof SettingUpdateSchema>;

export const SettingRevertSchema = z
  .object({
    targetVersion: z.number().int().nonnegative(),
    motif: NonEmpty(2000),
  })
  .strict();
export type SettingRevertRequest = z.infer<typeof SettingRevertSchema>;

export const AuditFiltersSchema = z
  .object({
    from: Iso8601.optional(),
    to: Iso8601.optional(),
    actor: NonEmpty(254).optional(),
    actions: z.array(NonEmpty(64)).max(32).optional(),
    ipCidr: NonEmpty(64).optional(),
    criticalOnly: z.boolean().optional(),
    page: z.number().int().nonnegative().optional(),
    size: z.number().int().positive().max(500).optional(),
  })
  .strict();
export type AuditFilters = z.infer<typeof AuditFiltersSchema>;

// ---------------------------------------------------------------------------
// 3. Break-Glass
// ---------------------------------------------------------------------------

export const BreakGlassRequestSchema = z
  .object({
    capability: z.enum(['db', 'grant', 'settings']),
    justification: NonEmpty(2000),
    otpCode: OtpCode,
    durationSeconds: z.number().int().positive().max(14_400).optional(),
  })
  .strict();
export type BreakGlassRequest = z.infer<typeof BreakGlassRequestSchema>;

export const BreakGlassRevokeSchema = z
  .object({
    motif: NonEmpty(2000),
  })
  .strict();
export type BreakGlassRevokeRequest = z.infer<typeof BreakGlassRevokeSchema>;

// ---------------------------------------------------------------------------
// 4. Recovery codes
// ---------------------------------------------------------------------------

export const RecoveryCodeGenerateSchema = z
  .object({
    motif: NonEmpty(2000),
    otpCode: OtpCode.optional(),
  })
  .strict();
export type RecoveryCodeGenerateRequest = z.infer<typeof RecoveryCodeGenerateSchema>;

export const RecoveryCodeUseSchema = z
  .object({
    userId: Uuid.optional(),
    code: OtpCode,
  })
  .strict();
export type RecoveryCodeUseRequest = z.infer<typeof RecoveryCodeUseSchema>;

// ---------------------------------------------------------------------------
// 5. WebAuthn / TOTP
// ---------------------------------------------------------------------------

export const WebAuthnRegisterBeginSchema = z
  .object({
    deviceLabel: NonEmpty(120).optional(),
  })
  .strict()
  .optional();

export const WebAuthnRegisterFinishSchema = z
  .object({
    challengeId: NonEmpty(256),
    deviceLabel: NonEmpty(120).optional(),
    attestation: z
      .object({
        id: NonEmpty(1024),
        rawId: NonEmpty(2048),
        type: z.literal('public-key'),
        response: z
          .object({
            clientDataJSON: NonEmpty(8192),
            attestationObject: NonEmpty(16384),
          })
          .strict(),
        clientExtensionResults: z.record(z.string(), z.unknown()).optional(),
      })
      .strict(),
  })
  .strict();
export type WebAuthnRegisterFinishRequest = z.infer<typeof WebAuthnRegisterFinishSchema>;

export const PasskeyRenameSchema = z
  .object({
    label: NonEmpty(120),
  })
  .strict();
export type PasskeyRenameRequest = z.infer<typeof PasskeyRenameSchema>;

export const TotpEnrollBeginSchema = z
  .object({
    label: NonEmpty(120).optional(),
  })
  .strict()
  .optional();

export const TotpEnrollFinishSchema = z
  .object({
    enrollmentId: NonEmpty(256),
    code: z.string().regex(/^\d{6,8}$/, 'TOTP code 6-8 digits'),
  })
  .strict();
export type TotpEnrollFinishRequest = z.infer<typeof TotpEnrollFinishSchema>;

// ---------------------------------------------------------------------------
// 6. Devices
// ---------------------------------------------------------------------------

export const TrustDeviceSchema = z
  .object({
    label: NonEmpty(120).optional(),
    ttlDays: z.number().int().positive().max(365).optional(),
  })
  .strict()
  .optional();

// ---------------------------------------------------------------------------
// 7. Self-management (delta 2026-04-30 §3)
// ---------------------------------------------------------------------------

export const MePasswordChangeSchema = z
  .object({
    currentPassword: z.string().min(8).max(255),
    newPassword: z.string().min(12).max(255),
  })
  .strict();
export type MePasswordChangeRequest = z.infer<typeof MePasswordChangeSchema>;

// ---------------------------------------------------------------------------
// 8. Account recovery (delta 2026-04-30 §4 + §5)
// ---------------------------------------------------------------------------

export const RecoveryInitiateSchema = z
  .object({
    email: z.string().email().max(254),
  })
  .strict();
export type RecoveryInitiateRequest = z.infer<typeof RecoveryInitiateSchema>;

export const RecoveryCompleteSchema = z
  .object({
    tokenOrCode: z.string().min(8).max(255),
    kratosFlowId: z.string().uuid().optional(),
  })
  .strict();
export type RecoveryCompleteRequest = z.infer<typeof RecoveryCompleteSchema>;

export const LoginRecoveryCodeSchema = z
  .object({
    kratosFlowId: z.string().uuid(),
    code: z.string().regex(/^[A-Z0-9]{4}-[A-Z0-9]{4}$/, 'recovery code format XXXX-XXXX'),
  })
  .strict();
export type LoginRecoveryCodeRequest = z.infer<typeof LoginRecoveryCodeSchema>;

// Phase 4.b.6 — Risk-based scoring entry. Body kept minimal: the BFF only
// passes `email`, the auth-ms re-derives the device fingerprint server-side
// from headers (UA + IP/24 + Accept-Language) — no client-supplied
// fingerprint to prevent spoofing.
export const LoginRiskAssessSchema = z
  .object({
    email: z.string().email().max(254),
  })
  .strict();
export type LoginRiskAssessRequest = z.infer<typeof LoginRiskAssessSchema>;

export const AdminRecoveryInitiateSchema = z
  .object({
    motif: z.string().min(50).max(2000),
    otpProof: z.string().regex(/^\d{8}$/, 'otpProof must be 8 digits'),
  })
  .strict();
export type AdminRecoveryInitiateRequest = z.infer<typeof AdminRecoveryInitiateSchema>;

// ---------------------------------------------------------------------------
// 8.b. Magic-link channel-binding (Phase 4.b.4)
// ---------------------------------------------------------------------------

export const OnboardBeginSchema = z
  .object({
    invitationId: z.string().uuid(),
    email: Email,
    role: AdminLevelSchema.optional(),
    inviterName: NonEmpty(120).optional(),
    inviterId: z.string().uuid().optional(),
    lang: z.enum(['fr', 'en']).optional(),
  })
  .strict();
export type OnboardBeginRequest = z.infer<typeof OnboardBeginSchema>;

export const OnboardVerifyLinkSchema = z
  .object({
    token: z.string().min(20).max(2048),
  })
  .strict();
export type OnboardVerifyLinkRequest = z.infer<typeof OnboardVerifyLinkSchema>;

export const OnboardVerifyOtpSchema = z
  .object({
    sessionId: z.string().uuid(),
    otpEntry: z.string().regex(/^\d{8}$/, 'otpEntry must be 8 digits'),
  })
  .strict();
export type OnboardVerifyOtpRequest = z.infer<typeof OnboardVerifyOtpSchema>;

export const RecoveryVerifyLinkSchema = z
  .object({
    token: z.string().min(20).max(2048),
  })
  .strict();
export type RecoveryVerifyLinkRequest = z.infer<typeof RecoveryVerifyLinkSchema>;

export const RecoveryVerifyOtpSchema = z
  .object({
    sessionId: z.string().uuid(),
    otpEntry: z.string().regex(/^\d{8}$/, 'otpEntry must be 8 digits'),
    kratosFlowId: z.string().uuid().optional(),
  })
  .strict();
export type RecoveryVerifyOtpRequest = z.infer<typeof RecoveryVerifyOtpSchema>;

// ---------------------------------------------------------------------------
// 9. Capabilities registry (delta 2026-04-30 §3.4)
// ---------------------------------------------------------------------------

export const CapabilityCheckUniquenessSchema = z
  .object({
    caps: z.array(z.string().min(1).max(120)).min(1).max(64),
    role: AdminLevelSchema,
  })
  .strict();
export type CapabilityCheckUniquenessRequest = z.infer<typeof CapabilityCheckUniquenessSchema>;

// ---------------------------------------------------------------------------
// 10. Push-approval WebSocket MFA (Phase 4.b.5)
// ---------------------------------------------------------------------------

export const PushApprovalInitiateSchema = z
  .object({
    userId: Uuid,
    ip: z.string().max(64).optional(),
    ua: z.string().max(512).optional(),
    city: z.string().max(120).optional(),
  })
  .strict();
export type PushApprovalInitiateRequest = z.infer<typeof PushApprovalInitiateSchema>;

export const PushApprovalRespondSchema = z
  .object({
    chosenNumber: z.number().int().min(0).max(9),
  })
  .strict();
export type PushApprovalRespondRequest = z.infer<typeof PushApprovalRespondSchema>;

// ---------------------------------------------------------------------------
// Step-up auth pour opérations sensibles (Phase 4.b.7)
// ---------------------------------------------------------------------------

export const StepUpMethodSchema = z.enum([
  'passkey',
  'push-approval',
  'totp',
  'otp',
]);
export type StepUpMethodWire = z.infer<typeof StepUpMethodSchema>;

/** Body de `POST /api/admin/auth/step-up/begin`. */
export const StepUpBeginSchema = z
  .object({
    requestedFor: NonEmpty(255),
  })
  .strict();
export type StepUpBeginRequest = z.infer<typeof StepUpBeginSchema>;

/** Body de `POST /api/admin/auth/step-up/{sessionId}/verify`. */
export const StepUpVerifySchema = z
  .object({
    method: StepUpMethodSchema,
    proof: NonEmpty(16384),
  })
  .strict();
export type StepUpVerifyRequest = z.infer<typeof StepUpVerifySchema>;

// Export central pour barrel import.
export const AdminSchemas = {
  AdminLevelSchema,
  InviteAdminSchema,
  GrantRoleRequestSchema,
  RevokeRoleRequestSchema,
  SuspendUserRequestSchema,
  MfaResetRequestSchema,
  OtpIssueSchema,
  OtpVerifySchema,
  SettingUpdateSchema,
  SettingRevertSchema,
  AuditFiltersSchema,
  BreakGlassRequestSchema,
  BreakGlassRevokeSchema,
  RecoveryCodeGenerateSchema,
  RecoveryCodeUseSchema,
  WebAuthnRegisterBeginSchema,
  WebAuthnRegisterFinishSchema,
  PasskeyRenameSchema,
  TotpEnrollBeginSchema,
  TotpEnrollFinishSchema,
  TrustDeviceSchema,
  MePasswordChangeSchema,
  RecoveryInitiateSchema,
  OnboardBeginSchema,
  OnboardVerifyLinkSchema,
  OnboardVerifyOtpSchema,
  RecoveryVerifyLinkSchema,
  RecoveryVerifyOtpSchema,
  RecoveryCompleteSchema,
  LoginRecoveryCodeSchema,
  LoginRiskAssessSchema,
  AdminRecoveryInitiateSchema,
  CapabilityCheckUniquenessSchema,
  PushApprovalInitiateSchema,
  PushApprovalRespondSchema,
  StepUpBeginSchema,
  StepUpVerifySchema,
};
