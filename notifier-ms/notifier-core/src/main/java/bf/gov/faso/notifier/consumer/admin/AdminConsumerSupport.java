/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.consumer.admin;

import bf.gov.faso.notifier.metrics.NotifierMetrics;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.kafka.core.KafkaTemplate;
import org.springframework.stereotype.Component;

import java.time.Duration;

/**
 * AdminConsumerSupport — utilities shared by all admin Kafka consumers:
 * <ul>
 *   <li>Idempotency check (KAYA SET NX with TTL)</li>
 *   <li>DLQ forwarding ({@code <topic>.dlq})</li>
 * </ul>
 */
@Component
public class AdminConsumerSupport {

    private static final Logger log = LoggerFactory.getLogger(AdminConsumerSupport.class);

    private final StringRedisTemplate kayaTemplate;
    private final KafkaTemplate<String, byte[]> kafkaTemplate;
    private final NotifierMetrics metrics;

    public AdminConsumerSupport(
            StringRedisTemplate kayaTemplate,
            KafkaTemplate<String, byte[]> kafkaTemplate,
            NotifierMetrics metrics) {
        this.kayaTemplate = kayaTemplate;
        this.kafkaTemplate = kafkaTemplate;
        this.metrics = metrics;
    }

    /**
     * Check-and-set idempotency marker. Returns {@code true} if this is the first
     * time we see {@code dedupKey} (caller should process), {@code false} on duplicate.
     */
    public boolean acquire(String dedupKey, Duration ttl) {
        try {
            Boolean ok = kayaTemplate.opsForValue().setIfAbsent(dedupKey, "1", ttl);
            if (Boolean.FALSE.equals(ok)) {
                metrics.incrementDedupeHit();
                return false;
            }
            return true;
        } catch (Exception e) {
            log.warn("KAYA dedup unavailable for key={} — proceeding (at-least-once): {}", dedupKey, e.getMessage());
            return true; // fail-open: do not block notifications on KAYA outage
        }
    }

    /** Forward a raw payload to the DLQ topic and bump the {@code notifier_dlq_total} counter. */
    public void forwardToDlq(String topic, String key, byte[] rawPayload) {
        String dlqTopic = topic + ".dlq";
        try {
            kafkaTemplate.send(dlqTopic, key, rawPayload);
            metrics.incrementDlq();
            log.warn("Forwarded to DLQ: topic={} key={}", dlqTopic, key);
        } catch (Exception e) {
            log.error("Failed to forward to DLQ topic={} key={} error={}", dlqTopic, key, e.getMessage());
        }
    }
}
