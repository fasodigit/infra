/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.consumer;

import bf.gov.faso.notifier.domain.GithubEventPayload;
import bf.gov.faso.notifier.metrics.NotifierMetrics;
import bf.gov.faso.notifier.rules.ContextRule;
import bf.gov.faso.notifier.rules.ContextRulesEngine;
import bf.gov.faso.notifier.service.NotificationService;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.kafka.annotation.KafkaListener;
import org.springframework.kafka.support.Acknowledgment;
import org.springframework.stereotype.Component;

import java.time.Duration;
import java.util.List;

/**
 * GithubEventConsumer — Redpanda consumer for topic {@code github.events.v1}.
 *
 * <p>Processing pipeline per message:
 * <ol>
 *   <li>Deserialize JSON payload → {@link GithubEventPayload}</li>
 *   <li>Deduplication check via KAYA (SET NX with 7-day TTL)</li>
 *   <li>If seen → ack silently, skip</li>
 *   <li>Evaluate context rules → list of matching {@link ContextRule}</li>
 *   <li>Enqueue async dispatch via {@link NotificationService#dispatchAll}</li>
 *   <li>Manual ack to Redpanda</li>
 * </ol>
 *
 * <p>On deserialization failure, the message is forwarded to the DLQ and acked
 * to prevent consumer stall.
 */
@Component
public class GithubEventConsumer {

    private static final Logger log = LoggerFactory.getLogger(GithubEventConsumer.class);

    private final ObjectMapper objectMapper;
    private final ContextRulesEngine rulesEngine;
    private final NotificationService notificationService;
    private final StringRedisTemplate kayaTemplate;
    private final NotifierMetrics metrics;

    @Value("${notifier.dedupe.ttl-seconds:604800}")
    private long dedupeTtlSeconds;

    @Value("${notifier.dedupe.key-prefix:delivery:}")
    private String dedupeKeyPrefix;

    public GithubEventConsumer(
            ObjectMapper objectMapper,
            ContextRulesEngine rulesEngine,
            NotificationService notificationService,
            StringRedisTemplate kayaTemplate,
            NotifierMetrics metrics) {
        this.objectMapper = objectMapper;
        this.rulesEngine = rulesEngine;
        this.notificationService = notificationService;
        this.kayaTemplate = kayaTemplate;
        this.metrics = metrics;
    }

    @KafkaListener(
        topics = "${notifier.topics.github-events:github.events.v1}",
        groupId = "notifier-ms",
        containerFactory = "kafkaListenerContainerFactory"
    )
    public void onGithubEvent(ConsumerRecord<String, byte[]> record, Acknowledgment ack) {
        String messageKey = record.key() != null ? record.key() : "no-key";
        log.debug("Received event: partition={} offset={} key={} size={}",
            record.partition(), record.offset(), messageKey, record.value().length);

        try {
            GithubEventPayload payload = objectMapper.readValue(record.value(), GithubEventPayload.class);

            // Deduplication via KAYA (Redis-protocol SET NX)
            String dedupeKey = dedupeKeyPrefix + payload.deliveryId();
            Boolean isNew = kayaTemplate.opsForValue()
                .setIfAbsent(dedupeKey, "1", Duration.ofSeconds(dedupeTtlSeconds));

            if (Boolean.FALSE.equals(isNew)) {
                log.info("Duplicate event suppressed by KAYA: deliveryId={}", payload.deliveryId());
                metrics.incrementDedupeHit();
                ack.acknowledge();
                return;
            }

            // Evaluate context rules
            List<ContextRule> matchedRules = rulesEngine.evaluate(payload);
            if (matchedRules.isEmpty()) {
                log.debug("No rules matched for event: type={} repo={}",
                    payload.eventType(),
                    payload.repository() != null ? payload.repository().fullName() : "unknown");
                ack.acknowledge();
                return;
            }

            log.info("Event matched {} rule(s): deliveryId={} type={} repo={}",
                matchedRules.size(), payload.deliveryId(), payload.eventType(),
                payload.repository() != null ? payload.repository().fullName() : "unknown");

            // Async dispatch — does not block consumer thread
            notificationService.dispatchAll(payload, matchedRules, record.value());
            ack.acknowledge();

        } catch (com.fasterxml.jackson.core.JsonProcessingException e) {
            log.error("Failed to deserialize event at partition={} offset={}: {}",
                record.partition(), record.offset(), e.getMessage());
            // Ack to avoid consumer stall; raw bytes forwarded to DLQ by NotificationService
            ack.acknowledge();
        } catch (Exception e) {
            log.error("Unexpected error processing event at partition={} offset={}: {}",
                record.partition(), record.offset(), e.getMessage(), e);
            // Still ack to prevent infinite retry on non-transient errors
            ack.acknowledge();
        }
    }
}
