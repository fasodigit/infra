// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.security.StepUpMethod;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Marks an admin-plane endpoint as requiring a fresh step-up authentication
 * (Phase 4.b.7 — {@code SECURITY-HARDENING-PLAN-2026-04-30 §4 Tier 4}).
 *
 * <p>The {@code StepUpAuthFilter} inspects the JWT claim
 * {@code last_step_up_at}; if absent or older than {@link #maxAgeSeconds}, the
 * filter responds with HTTP 401 + body
 * {@code { "error": "step_up_required", "methods_available": [...],
 *          "step_up_session_id": "...", "expires_at": "iso8601" }}.
 *
 * <p>The frontend reacts to this response by opening the
 * {@code <faso-step-up-guard>} modal and, after a successful verify, retries
 * the original request with the new short-lived JWT carrying the refreshed
 * {@code last_step_up_at} claim.
 */
@Target(ElementType.METHOD)
@Retention(RetentionPolicy.RUNTIME)
public @interface RequiresStepUp {

    /** Maximum acceptable age (in seconds) of the {@code last_step_up_at} JWT claim. */
    int maxAgeSeconds() default 300;

    /** Allowed step-up methods (priority order — first is preferred). */
    StepUpMethod[] allowedMethods() default {
            StepUpMethod.PASSKEY,
            StepUpMethod.PUSH_APPROVAL,
            StepUpMethod.TOTP,
            StepUpMethod.OTP
    };

    /**
     * Optional: only enforce step-up when one of these settings categories
     * is touched. Used by {@code AdminSettingsController.update} which is
     * sensitive only when the targeted category ∈ {audit, mfa, grant,
     * break_glass}. Empty array = always enforce.
     */
    String[] settingsCategories() default {};
}
