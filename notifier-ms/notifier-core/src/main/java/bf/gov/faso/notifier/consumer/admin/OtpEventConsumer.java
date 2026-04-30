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
 * OtpEventConsumer — Redpanda consumer for topic {@code auth.otp.issue}.
 *
 * <p>Expected JSON payload:
 * <pre>{@code
 * {
 *   "otpId": "uuid",
 *   "email": "user@example.bf",
 *   "code": "12345678",
 *   "expiresInMinutes": 5,
 *   "ipAddress": "10.0.0.1",
 *   "userAgent": "Mozilla/5.0...",
 *   "lang": "fr"
 * }
 * }</pre>
 *
 * <p>Idempotency: KAYA key {@code notifier:dedup:otp:<otpId>} with 1 h TTL.
 * Non-retriable failures (parse error, render error) → DLQ {@code auth.otp.issue.dlq}.
 */
@Component
public class OtpEventConsumer {

    private static final Logger log = LoggerFactory.getLogger(OtpEventConsumer.class);
    private static final String TEMPLATE = "admin/otp-email";
    private static final Duration DEDUP_TTL = Duration.ofHours(1);

    private final ObjectMapper objectMapper;
    private final AdminMailRenderer renderer;
    private final AdminMailDispatcher dispatcher;
    private final AdminConsumerSupport support;
    private final NotifierMetrics metrics;

    public OtpEventConsumer(
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
        topics = "${notifier.topics.otp-issue:auth.otp.issue}",
        groupId = "notifier-otp",
        containerFactory = "kafkaListenerContainerFactory"
    )
    public void onOtpIssue(ConsumerRecord<String, byte[]> record, Acknowledgment ack) {
        String topic = record.topic();
        String key = record.key() != null ? record.key() : "no-key";
        try {
            JsonNode payload = objectMapper.readTree(record.value());
            String otpId = textOrNull(payload, "otpId");
            String email = textOrNull(payload, "email");
            if (otpId == null || email == null) {
                log.warn("OTP event missing otpId/email — DLQ: partition={} offset={}",
                    record.partition(), record.offset());
                support.forwardToDlq(topic, key, record.value());
                ack.acknowledge();
                return;
            }

            String dedupKey = "notifier:dedup:otp:" + otpId;
            if (!support.acquire(dedupKey, DEDUP_TTL)) {
                log.info("Duplicate OTP event suppressed: otpId={}", otpId);
                ack.acknowledge();
                return;
            }

            Map<String, Object> vars = new HashMap<>();
            vars.put("code", textOrNull(payload, "code"));
            vars.put("expiresInMinutes", payload.has("expiresInMinutes") ? payload.get("expiresInMinutes").asInt(5) : 5);
            vars.put("ipAddress", textOrNull(payload, "ipAddress"));
            vars.put("userAgent", textOrNull(payload, "userAgent"));
            vars.put("lang", payload.has("lang") ? payload.get("lang").asText("fr") : "fr");

            AdminMailRenderer.RenderedAdminMail rendered = renderer.render(TEMPLATE, vars);
            dispatcher.send(email, rendered);
            metrics.incrementOtpSent();
            log.info("OTP email sent: otpId={} to={}", otpId, email);
            ack.acknowledge();

        } catch (com.fasterxml.jackson.core.JsonProcessingException e) {
            log.error("OTP deserialization failed: partition={} offset={} err={}",
                record.partition(), record.offset(), e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (AdminMailDispatcher.AdminMailDispatchException e) {
            log.error("OTP SMTP dispatch failed (will DLQ to allow operator replay): {}", e.getMessage());
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        } catch (Exception e) {
            log.error("Unexpected OTP processing error: {}", e.getMessage(), e);
            support.forwardToDlq(topic, key, record.value());
            ack.acknowledge();
        }
    }

    private static String textOrNull(JsonNode node, String field) {
        return node.has(field) && !node.get(field).isNull() ? node.get(field).asText() : null;
    }
}
