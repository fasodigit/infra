/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministere du Numerique, Burkina Faso
 */
package bf.gov.faso.notifier.config;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.kafka.core.KafkaTemplate;
import org.springframework.kafka.listener.CommonErrorHandler;
import org.springframework.kafka.listener.DeadLetterPublishingRecoverer;
import org.springframework.kafka.listener.DefaultErrorHandler;
import org.springframework.util.backoff.ExponentialBackOff;

/**
 * Kafka error handler with Dead Letter Queue (DLQ) support.
 *
 * <p>Failed messages are retried with exponential backoff (1s base, 2x multiplier,
 * max 30s elapsed, 3 attempts). After exhaustion, they are forwarded to the
 * DLQ topic ({@code <original-topic>.DLT}) via {@link DeadLetterPublishingRecoverer}.
 */
@Configuration
public class KafkaErrorHandlerConfig {

    private static final Logger log = LoggerFactory.getLogger(KafkaErrorHandlerConfig.class);

    @Bean
    public CommonErrorHandler errorHandler(KafkaTemplate<String, byte[]> kafkaTemplate) {
        var recoverer = new DeadLetterPublishingRecoverer(kafkaTemplate);
        var backoff = new ExponentialBackOff(1000L, 2.0);
        backoff.setMaxElapsedTime(30000L);
        backoff.setMaxAttempts(3);

        var handler = new DefaultErrorHandler(recoverer, backoff);
        handler.setRetryListeners((record, ex, deliveryAttempt) ->
                log.warn("Kafka retry attempt {} for topic={} key={}: {}",
                        deliveryAttempt, record.topic(), record.key(), ex.getMessage()));
        return handler;
    }
}
