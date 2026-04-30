/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.consumer.admin;

import bf.gov.faso.notifier.client.AdminDirectoryClient;
import bf.gov.faso.notifier.client.SlackWebhookClient;
import bf.gov.faso.notifier.metrics.NotifierMetrics;
import bf.gov.faso.notifier.service.admin.AdminMailDispatcher;
import bf.gov.faso.notifier.service.admin.AdminMailRenderer;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.kafka.annotation.KafkaListener;
import org.springframework.kafka.support.Acknowledgment;
import org.springframework.stereotype.Component;

import java.time.Duration;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * BreakGlassEventConsumer — Redpanda consumer for topic
 * {@code admin.break_glass.activated}.
 *
 * <p>Expected payload:
 * <pre>{@code
 * {
 *   "eventId": "uuid",
 *   "activator": "alice@faso.bf",
 *   "capability": "ROLE_GRANT_BYPASS",
 *   "justification": "P1 incident #42",
 *   "expiresAt": "2026-04-30T18:00:00Z",
 *   "traceId": "abc123",
 *   "lang": "fr"
 * }
 * }</pre>
 *
 * <p>Sends the alert to <strong>every active SUPER-ADMIN</strong> (resolved via
 * {@link AdminDirectoryClient}) and posts a stub Slack alert (TODO: wire Vault
 * webhook URL).
 */
@Component
public class BreakGlassEventConsumer {

    private static final Logger log = LoggerFactory.getLogger(BreakGlassEventConsumer.class);
    private static final String TEMPLATE = "admin/admin-break-glass-activated";
    private static final Duration DEDUP_TTL = Duration.ofDays(7);

    private final ObjectMapper objectMapper;
    private final AdminMailRenderer renderer;
    private final AdminMailDispatcher dispatcher;
    private final AdminConsumerSupport support;
    private final AdminDirectoryClient directory;
    private final SlackWebhookClient slack;
    private final NotifierMetrics metrics;

    public BreakGlassEventConsumer(
            ObjectMapper objectMapper,
            AdminMailRenderer renderer,
            AdminMailDispatcher dispatcher,
            AdminConsumerSupport support,
            AdminDirectoryClient directory,
            SlackWebhookClient slack,
            NotifierMetrics metrics) {
        this.objectMapper = objectMapper;
        this.renderer = renderer;
        this.dispatcher = dispatcher;
        this.support = support;
        this.directory = directory;
        this.slack = slack;
        this.metrics = metrics;
    }

    @KafkaListener(
        topics = "${notifier.topics.break-glass:admin.break_glass.activated}",
        groupId = "notifier-break-glass",
        containerFactory = "kafkaListenerContainerFactory"
    )
    public void onBreakGlassActivated(ConsumerRecord<String, byte[]> record, Acknowledgment ack) {
        String topic = record.topic();
        String key = record.key() != null ? record.key() : "no-key";

        try {
            JsonNode payload = objectMapper.readTree(record.value());
            String eventId = textOrNull(payload, "eventId");
            if (eventId == null) {
                log.warn("break_glass event missing eventId — DLQ");
                support.forwardToDlq(topic, key, record.value());
                ack.acknowledge();
                return;
            }

            String dedupKey = "notifier:dedup:break-glass:" + eventId;
            if (!support.acquire(dedupKey, DEDUP_TTL)) {
                log.info("Duplicate break_glass suppressed: eventId={}", eventId);
                ack.acknowledge();
                return;
            }

            String lang = payload.has("lang") ? payload.get("lang").asText("fr") : "fr";
            String activator = textOrNull(payload, "activator");
            String capability = textOrNull(payload, "capability");
            String justification = textOrNull(payload, "justification");
            String expiresAt = textOrNull(payload, "expiresAt");
            String traceId = textOrNull(payload, "traceId");

            Map<String, Object> vars = new HashMap<>();
            vars.put("activator", activator);
            vars.put("capability", capability);
            vars.put("justification", justification);
            vars.put("expiresAt", expiresAt);
            vars.put("traceId", traceId);
            vars.put("lang", lang);

            AdminMailRenderer.RenderedAdminMail rendered = renderer.render(TEMPLATE, vars);

            List<String> recipients = directory.listSuperAdminEmails();
            if (recipients.isEmpty()) {
                log.error("No active SUPER-ADMIN found — break_glass alert NOT delivered (eventId={})", eventId);
                support.forwardToDlq(topic, key, record.value());
                ack.acknowledge();
                return;
            }

            int sent = 0;
            for (String email : recipients) {
                try {
                    dispatcher.send(email, rendered);
                    sent++;
                } catch (Exception e) {
                    log.error("break_glass mail to {} failed: {}", email, e.getMessage());
                }
            }
            metrics.incrementBreakGlassSent();
            log.warn("BREAK-GLASS alert dispatched: eventId={} activator={} capability={} recipients={}/{}",
                eventId, activator, capability, sent, recipients.size());

            // Slack alert (best-effort, non-blocking)
            String slackText = String.format(
                ":rotating_light: *BREAK-GLASS* activated by `%s` — capability `%s` (expires %s) — trace=%s",
                activator, capability, expiresAt, traceId);
            slack.postSecurityAlert(slackText);

            ack.acknowledge();

        } catch (com.fasterxml.jackson.core.JsonProcessingException e) {
            log.error("break_glass deserialization failed: {}", e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (Exception e) {
            log.error("Unexpected break_glass processing error: {}", e.getMessage(), e);
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        }
    }

    private static String textOrNull(JsonNode node, String field) {
        return node.has(field) && !node.get(field).isNull() ? node.get(field).asText() : null;
    }
}
