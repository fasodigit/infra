/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.consumer.admin;

import bf.gov.faso.notifier.client.AdminDirectoryClient;
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
 * RecoveryEventConsumer — Redpanda consumer for the four account-recovery topics
 * (delta 2026-04-30, section 5):
 *
 * <ul>
 *   <li>{@code auth.recovery.self_initiated} — magic-link self recovery; renders
 *       {@code admin/admin-recovery-self-link} and dispatches to the target user.</li>
 *   <li>{@code auth.recovery.admin_initiated} — admin-initiated recovery; renders
 *       {@code admin/admin-recovery-admin-token} and dispatches to the target user.</li>
 *   <li>{@code auth.recovery.completed} — confirmation post-récupération;
 *       renders {@code admin/admin-recovery-completed}, sends to the user and
 *       broadcasts an audit copy to every active SUPER-ADMIN.</li>
 *   <li>{@code auth.recovery.used} — analytics-only event, logged at INFO,
 *       no email is dispatched.</li>
 * </ul>
 *
 * <p>Single multi-topic listener; dispatch is performed via {@link ConsumerRecord#topic()}.
 * KAYA dedup is keyed on {@code notifier:dedup:recovery:{eventId}} with a 1 h TTL.
 */
@Component
public class RecoveryEventConsumer {

    private static final Logger log = LoggerFactory.getLogger(RecoveryEventConsumer.class);

    private static final String TPL_SELF_LINK = "admin/admin-recovery-self-link";
    private static final String TPL_ADMIN_TOKEN = "admin/admin-recovery-admin-token";
    private static final String TPL_COMPLETED = "admin/admin-recovery-completed";

    private static final Duration DEDUP_TTL = Duration.ofHours(1);

    // Default topic names (matching application.yml fallback values).
    private static final String TOPIC_SELF_INITIATED = "auth.recovery.self_initiated";
    private static final String TOPIC_ADMIN_INITIATED = "auth.recovery.admin_initiated";
    private static final String TOPIC_COMPLETED = "auth.recovery.completed";
    private static final String TOPIC_USED = "auth.recovery.used";

    private final ObjectMapper objectMapper;
    private final AdminMailRenderer renderer;
    private final AdminMailDispatcher dispatcher;
    private final AdminConsumerSupport support;
    private final AdminDirectoryClient directory;
    private final NotifierMetrics metrics;

    public RecoveryEventConsumer(
            ObjectMapper objectMapper,
            AdminMailRenderer renderer,
            AdminMailDispatcher dispatcher,
            AdminConsumerSupport support,
            AdminDirectoryClient directory,
            NotifierMetrics metrics) {
        this.objectMapper = objectMapper;
        this.renderer = renderer;
        this.dispatcher = dispatcher;
        this.support = support;
        this.directory = directory;
        this.metrics = metrics;
    }

    @KafkaListener(
        topics = {
            "${notifier.topics.recovery-self-initiated:auth.recovery.self_initiated}",
            "${notifier.topics.recovery-admin-initiated:auth.recovery.admin_initiated}",
            "${notifier.topics.recovery-completed:auth.recovery.completed}",
            "${notifier.topics.recovery-used:auth.recovery.used}"
        },
        groupId = "notifier-recovery",
        containerFactory = "kafkaListenerContainerFactory"
    )
    public void onRecoveryEvent(ConsumerRecord<String, byte[]> record, Acknowledgment ack) {
        String topic = record.topic();
        String key = record.key() != null ? record.key() : "no-key";

        try {
            // auth.recovery.used → analytics only (no mail), short-circuit before parsing heavy fields.
            if (topic.endsWith(".used") || TOPIC_USED.equals(topic)) {
                handleUsed(record);
                ack.acknowledge();
                return;
            }

            JsonNode payload = objectMapper.readTree(record.value());
            String eventId = textOrNull(payload, "eventId");
            if (eventId == null) {
                log.warn("recovery event missing eventId on topic={} — DLQ", topic);
                support.forwardToDlq(topic, key, record.value());
                ack.acknowledge();
                return;
            }

            String dedupKey = "notifier:dedup:recovery:" + eventId;
            if (!support.acquire(dedupKey, DEDUP_TTL)) {
                log.info("Duplicate recovery event suppressed: topic={} eventId={}", topic, eventId);
                ack.acknowledge();
                return;
            }

            String lang = payload.has("lang") ? payload.get("lang").asText("fr") : "fr";

            if (topic.endsWith(".self_initiated") || TOPIC_SELF_INITIATED.equals(topic)) {
                handleSelfInitiated(payload, lang, eventId);
            } else if (topic.endsWith(".admin_initiated") || TOPIC_ADMIN_INITIATED.equals(topic)) {
                handleAdminInitiated(payload, lang, eventId);
            } else if (topic.endsWith(".completed") || TOPIC_COMPLETED.equals(topic)) {
                handleCompleted(payload, lang, eventId);
            } else {
                log.warn("Unhandled recovery topic: {} — acking without action", topic);
            }

            ack.acknowledge();

        } catch (com.fasterxml.jackson.core.JsonProcessingException e) {
            log.error("recovery event deserialization failed (topic={}): {}", topic, e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (AdminMailDispatcher.AdminMailDispatchException e) {
            log.error("recovery event SMTP dispatch failed (topic={}): {}", topic, e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (Exception e) {
            log.error("Unexpected recovery event processing error (topic={}): {}", topic, e.getMessage(), e);
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        }
    }

    // ── Per-topic handlers ──────────────────────────────────────────────────

    private void handleSelfInitiated(JsonNode payload, String lang, String eventId) {
        String userEmail = textOrNull(payload, "userEmail");
        String recoveryLink = textOrNull(payload, "recoveryLink");
        if (userEmail == null || recoveryLink == null) {
            log.warn("recovery.self_initiated missing userEmail/recoveryLink: eventId={}", eventId);
            return;
        }

        Map<String, Object> vars = new HashMap<>();
        vars.put("recoveryLink", recoveryLink);
        vars.put("expiresInMinutes", intOrDefault(payload, "expiresInMinutes", 30));
        vars.put("userFirstName", textOrNull(payload, "userFirstName"));
        vars.put("ipAddress", textOrNull(payload, "ipAddress"));
        vars.put("userAgent", textOrNull(payload, "userAgent"));
        vars.put("lang", lang);

        dispatcher.send(userEmail, renderer.render(TPL_SELF_LINK, vars));
        metrics.incrementRecoverySelfLinkSent();
        log.info("recovery.self_initiated mail sent: eventId={} to={}", eventId, userEmail);
    }

    private void handleAdminInitiated(JsonNode payload, String lang, String eventId) {
        String userEmail = textOrNull(payload, "userEmail");
        String recoveryToken = textOrNull(payload, "recoveryToken");
        if (userEmail == null || recoveryToken == null) {
            log.warn("recovery.admin_initiated missing userEmail/recoveryToken: eventId={}", eventId);
            return;
        }

        Map<String, Object> vars = new HashMap<>();
        vars.put("recoveryToken", recoveryToken);
        vars.put("expiresInMinutes", intOrDefault(payload, "expiresInMinutes", 60));
        vars.put("initiatorName", textOrNull(payload, "initiatorName"));
        vars.put("initiatorRole", textOrNull(payload, "initiatorRole"));
        vars.put("motif", textOrNull(payload, "motif"));
        vars.put("loginLink", textOrNull(payload, "loginLink"));
        vars.put("lang", lang);

        dispatcher.send(userEmail, renderer.render(TPL_ADMIN_TOKEN, vars));
        metrics.incrementRecoveryAdminTokenSent();
        log.info("recovery.admin_initiated mail sent: eventId={} to={} initiator={}",
            eventId, userEmail, vars.get("initiatorName"));
    }

    private void handleCompleted(JsonNode payload, String lang, String eventId) {
        String userEmail = textOrNull(payload, "userEmail");
        if (userEmail == null) {
            log.warn("recovery.completed missing userEmail: eventId={}", eventId);
            return;
        }

        Map<String, Object> vars = new HashMap<>();
        vars.put("recoveryType", textOrDefault(payload, "recoveryType", "SELF"));
        vars.put("completedAt", textOrNull(payload, "completedAt"));
        vars.put("ipAddress", textOrNull(payload, "ipAddress"));
        vars.put("newMfaMethod", textOrNull(payload, "newMfaMethod"));
        vars.put("traceId", textOrNull(payload, "traceId"));
        vars.put("lang", lang);

        AdminMailRenderer.RenderedAdminMail rendered = renderer.render(TPL_COMPLETED, vars);
        dispatcher.send(userEmail, rendered);
        metrics.incrementRecoveryCompletedSent();
        log.info("recovery.completed mail sent: eventId={} to={}", eventId, userEmail);

        // Audit broadcast — best-effort, does NOT block the user notification.
        // AdminDirectoryClient is fail-open (returns [] on error).
        try {
            List<String> superAdmins = directory.listSuperAdminEmails();
            int audited = 0;
            for (String saEmail : superAdmins) {
                if (saEmail == null || saEmail.isBlank() || saEmail.equalsIgnoreCase(userEmail)) continue;
                try {
                    dispatcher.send(saEmail, rendered);
                    audited++;
                } catch (Exception ex) {
                    log.warn("recovery.completed audit copy failed for {}: {}", saEmail, ex.getMessage());
                }
            }
            log.info("recovery.completed audit fan-out: eventId={} super_admins={}/{}",
                eventId, audited, superAdmins.size());
        } catch (Exception ex) {
            log.warn("recovery.completed audit fan-out skipped (directory unavailable): {}", ex.getMessage());
        }
    }

    private void handleUsed(ConsumerRecord<String, byte[]> record) {
        try {
            JsonNode payload = objectMapper.readTree(record.value());
            String eventId = textOrNull(payload, "eventId");
            String tokenType = textOrNull(payload, "tokenType");
            String userEmail = textOrNull(payload, "userEmail");
            log.info("recovery.used (analytics): eventId={} tokenType={} user={}",
                eventId, tokenType, userEmail);
        } catch (Exception e) {
            // Analytics consumer must never throw — log at debug and move on.
            log.debug("recovery.used payload not parseable: {}", e.getMessage());
        }
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
