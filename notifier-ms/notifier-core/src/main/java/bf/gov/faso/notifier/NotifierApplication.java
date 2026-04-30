/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.boot.autoconfigure.domain.EntityScan;
import org.springframework.data.jpa.repository.config.EnableJpaRepositories;
import org.springframework.kafka.annotation.EnableKafka;
import org.springframework.scheduling.annotation.EnableAsync;

/**
 * NotifierApplication — Entry point for the FASO DIGITALISATION notification microservice.
 *
 * <p>Consumes {@code github.events.v1} events from the Redpanda message bus,
 * resolves contextual templates per repository/event type, and dispatches
 * transactional emails via SMTP (MailHog in dev, Mailersend in prod).
 *
 * <p>Deduplication is performed via KAYA (SET NX / 7-day TTL) to ensure
 * idempotent delivery even on consumer restart or partition rebalance.
 */
@SpringBootApplication
@EntityScan(basePackages = {"bf.gov.faso.notifier", "bf.gov.faso.audit"})
@EnableJpaRepositories(basePackages = {"bf.gov.faso.notifier", "bf.gov.faso.audit"})
@EnableKafka
@EnableAsync
public class NotifierApplication {

    public static void main(String[] args) {
        SpringApplication.run(NotifierApplication.class, args);
    }
}
