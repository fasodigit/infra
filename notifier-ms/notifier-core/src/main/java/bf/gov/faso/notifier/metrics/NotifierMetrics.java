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

    private final Counter mailSentCounter;
    private final Counter mailFailedCounter;
    private final Counter dedupeHitCounter;
    private final Counter dlqCounter;
    private final Timer templateRenderTimer;

    public NotifierMetrics(MeterRegistry registry) {
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
    }

    public void incrementMailSent() { mailSentCounter.increment(); }
    public void incrementMailFailed() { mailFailedCounter.increment(); }
    public void incrementDedupeHit() { dedupeHitCounter.increment(); }
    public void incrementDlq() { dlqCounter.increment(); }
    public Timer templateRenderTimer() { return templateRenderTimer; }
}
