/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.metrics;

import io.micrometer.core.instrument.Counter;
import io.micrometer.core.instrument.MeterRegistry;
import io.micrometer.core.instrument.Timer;
import org.springframework.stereotype.Component;

/**
 * NotifierMetrics — Prometheus metrics for the notifier-ms service.
 *
 * <p>Exposes:
 * <ul>
 *   <li>{@code notifier_mail_sent_total} — successfully dispatched emails</li>
 *   <li>{@code notifier_mail_failed_total} — permanently failed deliveries (after retries)</li>
 *   <li>{@code notifier_template_render_duration_ms} — Handlebars render latency</li>
 *   <li>{@code notifier_dedupe_hit_total} — duplicate events suppressed by KAYA</li>
 *   <li>{@code notifier_dlq_total} — events forwarded to DLQ</li>
 * </ul>
 */
@Component
public class NotifierMetrics {

    private final MeterRegistry registry;
    private final Counter mailSentCounter;
    private final Counter mailFailedCounter;
    private final Counter dedupeHitCounter;
    private final Counter dlqCounter;
    private final Timer templateRenderTimer;

    // Admin (Phase 4.b) specialised counters
    private final Counter otpSentCounter;
    private final Counter roleGrantedSentCounter;
    private final Counter breakGlassSentCounter;
    private final Counter sessionRevokedSentCounter;

    // Account recovery (delta 2026-04-30, section 5)
    private final Counter recoverySelfLinkSentCounter;
    private final Counter recoveryAdminTokenSentCounter;
    private final Counter recoveryCompletedSentCounter;

    // Phase 4.b.4 — Magic-link channel-binding (admin onboarding)
    private final Counter onboardInvitationSentCounter;

    public NotifierMetrics(MeterRegistry registry) {
        this.registry = registry;
        this.mailSentCounter = Counter.builder("notifier_mail_sent_total")
            .description("Total number of emails successfully dispatched")
            .register(registry);

        this.mailFailedCounter = Counter.builder("notifier_mail_failed_total")
            .description("Total number of permanently failed email deliveries")
            .register(registry);

        this.dedupeHitCounter = Counter.builder("notifier_dedupe_hit_total")
            .description("Total number of duplicate events suppressed by KAYA deduplication")
            .register(registry);

        this.dlqCounter = Counter.builder("notifier_dlq_total")
            .description("Total number of events forwarded to DLQ after retry exhaustion")
            .register(registry);

        this.templateRenderTimer = Timer.builder("notifier_template_render_duration_ms")
            .description("Handlebars template render latency in milliseconds")
            .register(registry);

        this.otpSentCounter = Counter.builder("notifier_otp_sent_total")
            .description("Total number of admin OTP emails dispatched")
            .register(registry);

        this.roleGrantedSentCounter = Counter.builder("notifier_role_granted_sent_total")
            .description("Total number of admin role-granted notifications dispatched")
            .register(registry);

        this.breakGlassSentCounter = Counter.builder("notifier_break_glass_sent_total")
            .description("Total number of break-glass alert notifications dispatched")
            .register(registry);

        this.sessionRevokedSentCounter = Counter.builder("notifier_session_revoked_sent_total")
            .description("Total number of session-revoked notifications dispatched")
            .register(registry);

        this.recoverySelfLinkSentCounter = Counter.builder("notifier_recovery_self_link_sent_total")
            .description("Total number of self-service recovery (magic link) emails dispatched")
            .register(registry);

        this.recoveryAdminTokenSentCounter = Counter.builder("notifier_recovery_admin_token_sent_total")
            .description("Total number of admin-initiated recovery token emails dispatched")
            .register(registry);

        this.recoveryCompletedSentCounter = Counter.builder("notifier_recovery_completed_sent_total")
            .description("Total number of recovery-completion confirmation emails dispatched")
            .register(registry);

        this.onboardInvitationSentCounter = Counter.builder("notifier_onboard_invitation_sent_total")
            .description("Total number of admin-onboarding magic-link invitation emails dispatched")
            .register(registry);
    }

    public MeterRegistry registry() { return registry; }
    public void incrementMailSent() { mailSentCounter.increment(); }
    public void incrementMailFailed() { mailFailedCounter.increment(); }
    public void incrementDedupeHit() { dedupeHitCounter.increment(); }
    public void incrementDlq() { dlqCounter.increment(); }
    public Timer templateRenderTimer() { return templateRenderTimer; }

    public void incrementOtpSent() { otpSentCounter.increment(); }
    public void incrementRoleGrantedSent() { roleGrantedSentCounter.increment(); }
    public void incrementBreakGlassSent() { breakGlassSentCounter.increment(); }
    public void incrementSessionRevokedSent() { sessionRevokedSentCounter.increment(); }

    public void incrementRecoverySelfLinkSent() { recoverySelfLinkSentCounter.increment(); }
    public void incrementRecoveryAdminTokenSent() { recoveryAdminTokenSentCounter.increment(); }
    public void incrementRecoveryCompletedSent() { recoveryCompletedSentCounter.increment(); }

    public void incrementOnboardInvitationSent() { onboardInvitationSentCounter.increment(); }
}
