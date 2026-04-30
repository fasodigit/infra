// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.security;

/**
 * Allowed step-up authentication methods (Phase 4.b.7 — Tier 4).
 *
 * <p>Order in this enum matches the documented priority — strongest first
 * (cf. {@code SECURITY-HARDENING-PLAN-2026-04-30 §4 Tier 4} & §6).
 */
public enum StepUpMethod {

    /** FIDO2 user-verification re-touch — strongest, phishing-resistant. */
    PASSKEY,

    /** WebSocket push approval (Phase 4.b.5). */
    PUSH_APPROVAL,

    /** RFC 6238 TOTP code (6 digits). */
    TOTP,

    /** Email OTP 8-digit (fallback). */
    OTP;

    /** Wire-format key — kebab/lowercase used in JSON payloads. */
    public String wire() {
        return switch (this) {
            case PASSKEY -> "passkey";
            case PUSH_APPROVAL -> "push-approval";
            case TOTP -> "totp";
            case OTP -> "otp";
        };
    }

    /** Parse from wire-format key (case-insensitive, accepts hyphen/underscore). */
    public static StepUpMethod fromWire(String s) {
        if (s == null) throw new IllegalArgumentException("step-up method must not be null");
        String n = s.trim().toUpperCase().replace('-', '_');
        return StepUpMethod.valueOf(n);
    }
}
