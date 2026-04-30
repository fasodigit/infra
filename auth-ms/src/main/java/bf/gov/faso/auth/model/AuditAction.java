// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

/**
 * Centralised catalogue of audit action keys (string form persisted in
 * {@code audit_log.action}).
 *
 * <p>Existing call sites still use plain string literals (Phase 4.b iter 1 —
 * see {@link bf.gov.faso.auth.service.admin.AdminAuditService#log}); this enum
 * is the canonical reference for the delta amendment 2026-04-30 actions and
 * is consumed by the new services (CapabilityService, AccountRecoveryService,
 * SuperAdminProtectionService). Each enum value's {@link #key()} matches the
 * exact string written to DB.
 */
public enum AuditAction {

    // ── Delta 2026-04-30 ────────────────────────────────────────────────────
    SUPER_ADMIN_PROTECTION_TRIGGERED("super_admin.protection.triggered"),
    CAPABILITY_GRANTED("capability.granted"),
    CAPABILITY_REVOKED("capability.revoked"),
    CAPABILITY_SET_DUPLICATE_OVERRIDE("capability.set.duplicate_override"),
    ACCOUNT_RECOVERY_SELF_INITIATED("account.recovery.self_initiated"),
    ACCOUNT_RECOVERY_ADMIN_INITIATED("account.recovery.admin_initiated"),
    ACCOUNT_RECOVERY_COMPLETED("account.recovery.completed"),
    RECOVERY_CODE_INVALID("recovery_code.invalid"),
    RECOVERY_CODE_USED("recovery_code.used"),
    SELF_PASSWORD_CHANGED("self.password.changed"),
    SELF_PASSKEY_ENROLLED("self.passkey.enrolled"),
    SELF_PASSKEY_REVOKED("self.passkey.revoked"),
    SELF_TOTP_ENROLLED("self.totp.enrolled"),
    SELF_TOTP_DISABLED("self.totp.disabled"),
    SELF_RECOVERY_CODES_REGENERATED("self.recovery_codes.regenerated"),

    // ── Phase 4.b.4 — Magic-link channel-binding ────────────────────────────
    MAGIC_LINK_ISSUED("magic_link.issued"),
    MAGIC_LINK_VERIFIED("magic_link.verified"),
    MAGIC_LINK_REPLAYED("magic_link.replayed"),
    ONBOARD_COMPLETED("onboard.completed"),

    // ── Phase 4.b.7 — Step-up auth for sensitive operations ─────────────────
    STEP_UP_REQUESTED("step_up.requested"),
    STEP_UP_VERIFIED("step_up.verified"),
    STEP_UP_FAILED("step_up.failed"),

    // ── Phase 4.b.6 — Risk-based scoring MVP ───────────────────────────────
    LOGIN_RISK_ASSESSED("login.risk.assessed"),
    LOGIN_BLOCKED_HIGH_RISK("login.blocked.high_risk"),
    LOGIN_STEP_UP_REQUIRED("login.step_up.required"),

    // ── Phase 4.b.5 — Push approval WebSocket MFA ──────────────────────────
    PUSH_APPROVAL_REQUESTED("push_approval.requested"),
    PUSH_APPROVAL_GRANTED("push_approval.granted"),
    PUSH_APPROVAL_DENIED("push_approval.denied"),
    PUSH_APPROVAL_TIMEOUT("push_approval.timeout"),
    PUSH_APPROVAL_NUMBER_MISMATCH("push_approval.number_mismatch"),

    // ── Phase 4.b.3 — Crypto upgrade Argon2id + HMAC pepper ─────────────────
    /**
     * Emitted (silently, metric-only) when {@code LoginRehashService} upgrades
     * a stored password hash from a legacy algorithm or stale pepper version
     * to the current Argon2id profile during a successful login.
     */
    HASH_REHASHED_ON_LOGIN("hash.rehashed_on_login");

    private final String key;

    AuditAction(String key) { this.key = key; }

    public String key() { return key; }

    @Override public String toString() { return key; }
}
