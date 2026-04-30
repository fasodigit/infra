/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.consumer.admin;

import bf.gov.faso.notifier.client.AdminSettingsClient;
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
 * SessionRevokedEventConsumer — Redpanda consumer for topic
 * {@code auth.session.revoked}.
 *
 * <p>Notification is gated by the admin setting
 * {@code mfa.notify_user_on_session_revoke} (cached 30 s in KAYA via
 * {@link AdminSettingsClient}).
 *
 * <p>Expected payload:
 * <pre>{@code
 * {
 *   "sessionId": "uuid",
 *   "userEmail": "...",
 *   "revokedBy": "alice@faso.bf",
 *   "ipAddress": "10.0.0.1",
 *   "lang": "fr"
 * }
 * }</pre>
 */
@Component
public class SessionRevokedEventConsumer {

    private static final Logger log = LoggerFactory.getLogger(SessionRevokedEventConsumer.class);
    private static final String TEMPLATE = "admin/admin-session-revoked";
    private static final String SETTING_KEY = "mfa.notify_user_on_session_revoke";
    private static final Duration DEDUP_TTL = Duration.ofHours(6);

    private final ObjectMapper objectMapper;
    private final AdminMailRenderer renderer;
    private final AdminMailDispatcher dispatcher;
    private final AdminConsumerSupport support;
    private final AdminSettingsClient settings;
    private final NotifierMetrics metrics;

    public SessionRevokedEventConsumer(
            ObjectMapper objectMapper,
            AdminMailRenderer renderer,
            AdminMailDispatcher dispatcher,
            AdminConsumerSupport support,
            AdminSettingsClient settings,
            NotifierMetrics metrics) {
        this.objectMapper = objectMapper;
        this.renderer = renderer;
        this.dispatcher = dispatcher;
        this.support = support;
        this.settings = settings;
        this.metrics = metrics;
    }

    @KafkaListener(
        topics = "${notifier.topics.session-revoked:auth.session.revoked}",
        groupId = "notifier-session-revoked",
        containerFactory = "kafkaListenerContainerFactory"
    )
    public void onSessionRevoked(ConsumerRecord<String, byte[]> record, Acknowledgment ack) {
        String topic = record.topic();
        String key = record.key() != null ? record.key() : "no-key";

        try {
            // Settings gate (default: true)
            boolean notifyUser = settings.getBoolean(SETTING_KEY, true);
            if (!notifyUser) {
                log.debug("Setting {}=false → silently acking session.revoked event", SETTING_KEY);
                ack.acknowledge();
                return;
            }

            JsonNode payload = objectMapper.readTree(record.value());
            String sessionId = textOrNull(payload, "sessionId");
            String userEmail = textOrNull(payload, "userEmail");
            if (sessionId == null || userEmail == null) {
                log.warn("session.revoked missing sessionId/userEmail — DLQ");
                support.forwardToDlq(topic, key, record.value());
                ack.acknowledge();
                return;
            }

            String dedupKey = "notifier:dedup:session-revoked:" + sessionId;
            if (!support.acquire(dedupKey, DEDUP_TTL)) {
                log.info("Duplicate session.revoked suppressed: sessionId={}", sessionId);
                ack.acknowledge();
                return;
            }

            Map<String, Object> vars = new HashMap<>();
            vars.put("revokedBy", textOrNull(payload, "revokedBy"));
            vars.put("ipAddress", textOrNull(payload, "ipAddress"));
            vars.put("lang", payload.has("lang") ? payload.get("lang").asText("fr") : "fr");

            AdminMailRenderer.RenderedAdminMail rendered = renderer.render(TEMPLATE, vars);
            dispatcher.send(userEmail, rendered);
            metrics.incrementSessionRevokedSent();
            log.info("session.revoked notification sent: sessionId={} to={}", sessionId, userEmail);
            ack.acknowledge();

        } catch (com.fasterxml.jackson.core.JsonProcessingException e) {
            log.error("session.revoked deserialization failed: {}", e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (AdminMailDispatcher.AdminMailDispatchException e) {
            log.error("session.revoked SMTP dispatch failed: {}", e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (Exception e) {
            log.error("Unexpected session.revoked processing error: {}", e.getMessage(), e);
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        }
    }

    private static String textOrNull(JsonNode node, String field) {
        return node.has(field) && !node.get(field).isNull() ? node.get(field).asText() : null;
    }
}
