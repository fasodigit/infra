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
 * RoleGrantedEventConsumer — Redpanda consumer for topic {@code auth.role.granted}.
 *
 * <p>Expected JSON payload:
 * <pre>{@code
 * {
 *   "eventId": "uuid",
 *   "targetEmail": "...",
 *   "targetRole": "OPERATOR",
 *   "grantor": "alice@faso.bf",
 *   "justification": "Onboarding new operator",
 *   "scope": "tenant:agriculture",
 *   "dualControl": true,
 *   "approverEmail": "bob@faso.bf",
 *   "approvalLink": "https://admin.faso.bf/approvals/...",
 *   "lang": "fr"
 * }
 * }</pre>
 *
 * <p>Behaviour:
 * <ul>
 *   <li>Always notifies {@code targetEmail} via {@code admin-role-granted.hbs}</li>
 *   <li>If {@code dualControl=true}, also notifies {@code approverEmail} via
 *       {@code admin-role-grant-approval-required.hbs}</li>
 * </ul>
 */
@Component
public class RoleGrantedEventConsumer {

    private static final Logger log = LoggerFactory.getLogger(RoleGrantedEventConsumer.class);
    private static final String TPL_TARGET = "admin/admin-role-granted";
    private static final String TPL_APPROVER = "admin/admin-role-grant-approval-required";
    private static final Duration DEDUP_TTL = Duration.ofHours(24);

    private final ObjectMapper objectMapper;
    private final AdminMailRenderer renderer;
    private final AdminMailDispatcher dispatcher;
    private final AdminConsumerSupport support;
    private final NotifierMetrics metrics;

    public RoleGrantedEventConsumer(
            ObjectMapper objectMapper,
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
        topics = "${notifier.topics.role-granted:auth.role.granted}",
        groupId = "notifier-role-granted",
        containerFactory = "kafkaListenerContainerFactory"
    )
    public void onRoleGranted(ConsumerRecord<String, byte[]> record, Acknowledgment ack) {
        String topic = record.topic();
        String key = record.key() != null ? record.key() : "no-key";

        try {
            JsonNode payload = objectMapper.readTree(record.value());
            String eventId = textOrNull(payload, "eventId");
            String targetEmail = textOrNull(payload, "targetEmail");
            if (eventId == null || targetEmail == null) {
                log.warn("role.granted event missing eventId/targetEmail — DLQ");
                support.forwardToDlq(topic, key, record.value());
                ack.acknowledge();
                return;
            }

            String dedupKey = "notifier:dedup:role-granted:" + eventId;
            if (!support.acquire(dedupKey, DEDUP_TTL)) {
                log.info("Duplicate role.granted suppressed: eventId={}", eventId);
                ack.acknowledge();
                return;
            }

            String lang = payload.has("lang") ? payload.get("lang").asText("fr") : "fr";

            Map<String, Object> targetVars = new HashMap<>();
            targetVars.put("targetRole", textOrNull(payload, "targetRole"));
            targetVars.put("grantor", textOrNull(payload, "grantor"));
            targetVars.put("justification", textOrNull(payload, "justification"));
            targetVars.put("scope", textOrNull(payload, "scope"));
            targetVars.put("lang", lang);
            dispatcher.send(targetEmail, renderer.render(TPL_TARGET, targetVars));
            metrics.incrementRoleGrantedSent();
            log.info("role.granted notification sent: eventId={} to={}", eventId, targetEmail);

            boolean dualControl = payload.has("dualControl") && payload.get("dualControl").asBoolean(false);
            String approverEmail = textOrNull(payload, "approverEmail");
            if (dualControl && approverEmail != null) {
                Map<String, Object> approverVars = new HashMap<>();
                approverVars.put("requester", textOrNull(payload, "grantor"));
                approverVars.put("target", targetEmail);
                approverVars.put("role", textOrNull(payload, "targetRole"));
                approverVars.put("justification", textOrNull(payload, "justification"));
                approverVars.put("approvalLink", textOrNull(payload, "approvalLink"));
                approverVars.put("lang", lang);
                dispatcher.send(approverEmail, renderer.render(TPL_APPROVER, approverVars));
                log.info("role.granted approval-request sent: eventId={} approver={}", eventId, approverEmail);
            }

            ack.acknowledge();

        } catch (com.fasterxml.jackson.core.JsonProcessingException e) {
            log.error("role.granted deserialization failed: {}", e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (AdminMailDispatcher.AdminMailDispatchException e) {
            log.error("role.granted SMTP dispatch failed: {}", e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (Exception e) {
            log.error("Unexpected role.granted processing error: {}", e.getMessage(), e);
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        }
    }

    private static String textOrNull(JsonNode node, String field) {
        return node.has(field) && !node.get(field).isNull() ? node.get(field).asText() : null;
    }
}
