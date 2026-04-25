// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit;

import io.micrometer.core.instrument.Counter;
import io.micrometer.core.instrument.MeterRegistry;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.scheduling.annotation.Async;
import org.springframework.stereotype.Service;

/**
 * Async audit writer — persists {@link AuditEvent} rows and emits structured
 * logs (label: {@code log_type=audit}) for Loki ingestion.
 *
 * <p>Failures are logged but never re-thrown so that audit errors do not
 * break the business flow.
 */
@Service
public class AuditService {

    private static final Logger log = LoggerFactory.getLogger(AuditService.class);

    private final AuditRepository repository;
    private final Counter auditCounter;

    @Value("${spring.application.name:unknown}")
    private String serviceName;

    public AuditService(AuditRepository repository, MeterRegistry registry) {
        this.repository = repository;
        this.auditCounter = Counter.builder("faso_audit_events_total")
                .description("Total audit events recorded")
                .register(registry);
    }

    /**
     * Persist an audit event asynchronously.
     *
     * <p>The {@code serviceName} field is always overwritten with
     * {@code spring.application.name} so that callers cannot spoof it.
     *
     * @param event the audit event to record (must not be {@code null})
     */
    @Async
    public void record(AuditEvent event) {
        try {
            AuditEvent persisted = AuditEvent.builder()
                    .actorId(event.getActorId())
                    .actorType(event.getActorType())
                    .action(event.getAction())
                    .resourceType(event.getResourceType())
                    .resourceId(event.getResourceId())
                    .ipAddress(event.getIpAddress())
                    .userAgent(event.getUserAgent())
                    .result(event.getResult())
                    .metadata(event.getMetadata())
                    .traceId(event.getTraceId())
                    .serviceName(serviceName)
                    .build();
            repository.save(persisted);
            auditCounter.increment();

            // Also emit structured log for Loki (label: log_type=audit)
            log.info("audit_event action={} actor={} resource={}:{} result={}",
                    persisted.getAction(), persisted.getActorId(),
                    persisted.getResourceType(), persisted.getResourceId(),
                    persisted.getResult());
        } catch (Exception e) {
            log.error("Failed to record audit event: {}", e.getMessage(), e);
        }
    }
}
