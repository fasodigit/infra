// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit;

import io.micrometer.core.instrument.Counter;
import io.micrometer.core.instrument.MeterRegistry;
import io.micrometer.tracing.Tracer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.slf4j.MDC;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.scheduling.annotation.Async;
import org.springframework.stereotype.Service;

/**
 * Async audit writer — persists {@link AuditEvent} rows and emits structured
 * logs (label: {@code log_type=audit}) for Loki ingestion.
 *
 * <p>Two responsibilities:
 * <ol>
 *   <li>Persist the event in {@code audit.audit_log} (partitioned table).</li>
 *   <li>Emit a single-line structured log with MDC fields
 *       {@code log_type=audit}, {@code action}, {@code actor},
 *       {@code result}, {@code service}, {@code traceId}. The Logback
 *       JSON encoder picks these up and Loki tenant routing
 *       ({@code faso-audit} 5-year retention) matches on
 *       {@code log_type="audit"}.</li>
 * </ol>
 *
 * <p>{@code traceId} is sourced from {@link Tracer} when available so the
 * audit row can be joined to the originating distributed trace in Tempo.
 *
 * <p>Failures are logged but never re-thrown — audit errors must not break
 * the business flow.
 */
@Service
public class AuditService {

    private static final Logger log = LoggerFactory.getLogger(AuditService.class);

    private static final String MDC_LOG_TYPE = "log_type";
    private static final String MDC_AUDIT = "audit";

    private final AuditRepository repository;
    private final Counter auditCounter;
    private final Tracer tracer;

    @Value("${spring.application.name:unknown}")
    private String serviceName;

    @Autowired(required = false)
    public AuditService(AuditRepository repository, MeterRegistry registry, Tracer tracer) {
        this.repository = repository;
        this.tracer = tracer;
        this.auditCounter = Counter.builder("faso_audit_events_total")
                .description("Total audit events recorded")
                .register(registry);
    }

    /**
     * Fallback constructor for services without micrometer-tracing on the
     * classpath. {@code traceId} will be left null in that case.
     */
    public AuditService(AuditRepository repository, MeterRegistry registry) {
        this(repository, registry, null);
    }

    /**
     * Persist an audit event asynchronously.
     *
     * <p>The {@code serviceName} field is always overwritten with
     * {@code spring.application.name} so callers cannot spoof it.
     * {@code traceId} is populated from {@link Tracer} if the caller did not
     * provide one explicitly.
     *
     * @param event the audit event to record (must not be {@code null})
     */
    @Async("auditTaskExecutor")
    public void record(AuditEvent event) {
        try {
            String traceId = event.getTraceId() != null
                    ? event.getTraceId()
                    : currentTraceId();

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
                    .traceId(traceId)
                    .serviceName(serviceName)
                    .build();
            repository.save(persisted);
            auditCounter.increment();

            // Structured log for Loki — MDC log_type=audit drives the
            // 5-year-retention tenant routing in loki-overrides.yaml.
            // The MDC entry is restored to its previous state in finally.
            String prevLogType = MDC.get(MDC_LOG_TYPE);
            try {
                MDC.put(MDC_LOG_TYPE, MDC_AUDIT);
                if (traceId != null) {
                    MDC.put("traceId", traceId);
                }
                log.info("audit_event action={} actor={} resource={}:{} result={} service={}",
                        persisted.getAction(),
                        persisted.getActorId(),
                        persisted.getResourceType(),
                        persisted.getResourceId(),
                        persisted.getResult(),
                        persisted.getServiceName());
            } finally {
                if (prevLogType == null) {
                    MDC.remove(MDC_LOG_TYPE);
                } else {
                    MDC.put(MDC_LOG_TYPE, prevLogType);
                }
                MDC.remove("traceId");
            }
        } catch (Exception e) {
            log.error("Failed to record audit event: {}", e.getMessage(), e);
        }
    }

    private String currentTraceId() {
        if (tracer == null) {
            return null;
        }
        try {
            var current = tracer.currentSpan();
            return current != null ? current.context().traceId() : null;
        } catch (Exception ignored) {
            return null;
        }
    }
}
