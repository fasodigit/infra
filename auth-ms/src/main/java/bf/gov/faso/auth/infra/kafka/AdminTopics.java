// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.infra.kafka;

/**
 * Centralised list of the 9 Redpanda topics produced by auth-ms (Phase 4.b).
 * Names match the gap-analysis §6.
 */
public final class AdminTopics {
    private AdminTopics() {}

    public static final String OTP_ISSUE                  = "auth.otp.issue";
    public static final String OTP_VERIFIED               = "auth.otp.verified";
    public static final String ROLE_GRANTED               = "auth.role.granted";
    public static final String ROLE_REVOKED               = "auth.role.revoked";
    public static final String DEVICE_TRUSTED             = "auth.device.trusted";
    public static final String SESSION_REVOKED            = "auth.session.revoked";
    public static final String BREAK_GLASS_ACTIVATED      = "admin.break_glass.activated";
    public static final String SETTINGS_CHANGED           = "admin.settings.changed";
    public static final String USER_SUSPENDED             = "admin.user.suspended";
    public static final String USER_REACTIVATED           = "admin.user.reactivated";
    public static final String AUDIT_EVENT                = "admin.audit.event";

    // ── Delta amendment 2026-04-30 ─────────────────────────────────────────
    public static final String RECOVERY_SELF_INITIATED    = "auth.recovery.self_initiated";
    public static final String RECOVERY_ADMIN_INITIATED   = "auth.recovery.admin_initiated";
    public static final String RECOVERY_COMPLETED         = "auth.recovery.completed";
    public static final String RECOVERY_USED              = "auth.recovery.used";
    public static final String CAPABILITY_GRANTED         = "auth.capability.granted";
    public static final String CAPABILITY_REVOKED         = "auth.capability.revoked";

    // ── Phase 4.b.5 — Push approval (WebSocket sovereign MFA) ─────────────
    public static final String AUTH_PUSH_REQUESTED        = "auth.push.requested";
    public static final String AUTH_PUSH_GRANTED          = "auth.push.granted";
    public static final String AUTH_PUSH_DENIED           = "auth.push.denied";
    public static final String AUTH_PUSH_TIMEOUT          = "auth.push.timeout";
    public static final String AUTH_PUSH_NUMBER_MISMATCH  = "auth.push.number_mismatch";

    // ── Phase 4.b.4 — Magic-link channel-binding ────────────────────────────
    public static final String AUTH_ONBOARD_INVITATION_SENT = "auth.onboard.invitation_sent";
    public static final String AUTH_ONBOARD_COMPLETED       = "auth.onboard.completed";

    // ── Phase 4.b.6 — Risk-based scoring MVP ────────────────────────────────
    /** Per-login risk assessment (every score, decision, signals[]).
     *  Recommended Redpanda config: 3 partitions, retention 30d. */
    public static final String AUTH_RISK_ASSESSED          = "auth.risk.assessed";
    /** High-risk login outcomes (BLOCK only) — feed to threat-intel / SIEM.
     *  Recommended Redpanda config: 1 partition, retention 90d. */
    public static final String AUTH_RISK_BLOCKED           = "auth.risk.blocked";

    // ── Phase 4.b.7 — Step-up auth for sensitive operations ─────────────────
    /** Step-up session opened (filter caught a stale {@code last_step_up_at}). */
    public static final String AUTH_STEP_UP_REQUESTED      = "auth.step_up.requested";
    /** Step-up successfully verified — short-lived JWT issued with refreshed
     *  {@code last_step_up_at} claim. */
    public static final String AUTH_STEP_UP_VERIFIED       = "auth.step_up.verified";
    /** Step-up verification failed (wrong proof / expired session / lock). */
    public static final String AUTH_STEP_UP_FAILED         = "auth.step_up.failed";
}
