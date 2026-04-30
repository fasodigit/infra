/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.consumer.admin;

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
import java.util.Map;

/**
 * OnboardingEventConsumer — Phase 4.b.4 magic-link channel-binding.
 *
 * <p>Consumes the {@code auth.onboard.invitation_sent} Redpanda topic published
 * by auth-ms when a SUPER-ADMIN issues an admin invitation. Renders the
 * {@code admin/admin-onboard-magic-link.hbs} template and dispatches the
 * email through {@link AdminMailDispatcher}.
 *
 * <p>The {@code auth.onboard.completed} topic is logged at INFO for analytics
 * but does NOT trigger a confirmation email (the user is already inside the
 * MFA-enrolment funnel and will receive the standard recovery-codes email
 * once enrolment finishes).
 *
 * <p>KAYA dedup is keyed on {@code notifier:dedup:onboard:{eventId}} TTL 1 h.
 */
@Component
public class OnboardingEventConsumer {

    private static final Logger log = LoggerFactory.getLogger(OnboardingEventConsumer.class);

    private static final String TPL_INVITATION = "admin/admin-onboard-magic-link";
    private static final Duration DEDUP_TTL = Duration.ofHours(1);

    private static final String TOPIC_INVITATION_SENT = "auth.onboard.invitation_sent";
    private static final String TOPIC_COMPLETED       = "auth.onboard.completed";

    private final ObjectMapper objectMapper;
    private final AdminMailRenderer renderer;
    private final AdminMailDispatcher dispatcher;
    private final AdminConsumerSupport support;
    private final NotifierMetrics metrics;

    public OnboardingEventConsumer(ObjectMapper objectMapper,
                                   AdminMailRenderer renderer,
                                   AdminMailDispatcher dispatcher,
                                   AdminConsumerSupport support,
                                   NotifierMetrics metrics) {
        this.objectMapper = objectMapper;
        this.renderer = renderer;
        this.dispatcher = dispatcher;
        this.support = support;
        this.metrics = metrics;
    }

    @KafkaListener(
        topics = {
            "${notifier.topics.onboard-invitation-sent:auth.onboard.invitation_sent}",
            "${notifier.topics.onboard-completed:auth.onboard.completed}"
        },
        groupId = "notifier-onboarding",
        containerFactory = "kafkaListenerContainerFactory"
    )
    public void onOnboardEvent(ConsumerRecord<String, byte[]> record, Acknowledgment ack) {
        String topic = record.topic();
        String key = record.key() != null ? record.key() : "no-key";

        try {
            JsonNode payload = objectMapper.readTree(record.value());
            String eventId = textOrNull(payload, "eventId");
            if (eventId == null) {
                log.warn("onboard event missing eventId on topic={} — DLQ", topic);
                support.forwardToDlq(topic, key, record.value());
                ack.acknowledge();
                return;
            }

            String dedupKey = "notifier:dedup:onboard:" + eventId;
            if (!support.acquire(dedupKey, DEDUP_TTL)) {
                log.info("Duplicate onboard event suppressed: topic={} eventId={}", topic, eventId);
                ack.acknowledge();
                return;
            }

            if (topic.endsWith(".invitation_sent") || TOPIC_INVITATION_SENT.equals(topic)) {
                handleInvitationSent(payload, eventId);
            } else if (topic.endsWith(".completed") || TOPIC_COMPLETED.equals(topic)) {
                handleCompleted(payload, eventId);
            } else {
                log.warn("Unhandled onboard topic: {} — acking without action", topic);
            }

            ack.acknowledge();
        } catch (com.fasterxml.jackson.core.JsonProcessingException e) {
            log.error("onboard event deserialization failed (topic={}): {}", topic, e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (AdminMailDispatcher.AdminMailDispatchException e) {
            log.error("onboard SMTP dispatch failed (topic={}): {}", topic, e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (Exception e) {
            log.error("Unexpected onboard event processing error (topic={}): {}",
                    topic, e.getMessage(), e);
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        }
    }

    private void handleInvitationSent(JsonNode envelope, String eventId) {
        JsonNode payload = envelope.has("payload") ? envelope.get("payload") : envelope;
        String userEmail = textOrNull(payload, "userEmail");
        String magicLink = textOrNull(payload, "magicLink");
        if (userEmail == null || magicLink == null) {
            log.warn("onboard.invitation_sent missing userEmail/magicLink: eventId={}", eventId);
            return;
        }

        String lang = textOrDefault(payload, "lang", "fr");

        Map<String, Object> vars = new HashMap<>();
        vars.put("magicLink", magicLink);
        vars.put("expiresInMinutes", intOrDefault(payload, "expiresInMinutes", 30));
        vars.put("inviterName", textOrDefault(payload, "inviterName", ""));
        vars.put("targetRole", textOrDefault(payload, "targetRole", ""));
        vars.put("ipAddress", textOrDefault(payload, "ipAddress", ""));
        vars.put("lang", lang);

        dispatcher.send(userEmail, renderer.render(TPL_INVITATION, vars));
        metrics.incrementOnboardInvitationSent();
        log.info("onboard.invitation_sent mail sent: eventId={} to={}", eventId, userEmail);
    }

    private void handleCompleted(JsonNode envelope, String eventId) {
        JsonNode payload = envelope.has("payload") ? envelope.get("payload") : envelope;
        String userEmail = textOrNull(payload, "userEmail");
        String userId = textOrNull(payload, "userId");
        // Analytics-only (no mail) — the user is in the MFA enrolment flow.
        log.info("onboard.completed (analytics): eventId={} userId={} email={}",
                eventId, userId, userEmail);
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    private static String textOrNull(JsonNode node, String field) {
        return node.has(field) && !node.get(field).isNull() ? node.get(field).asText() : null;
    }

    private static String textOrDefault(JsonNode node, String field, String def) {
        String v = textOrNull(node, field);
        return v != null ? v : def;
    }

    private static int intOrDefault(JsonNode node, String field, int def) {
        return node.has(field) && node.get(field).isNumber() ? node.get(field).asInt(def) : def;
    }
}
