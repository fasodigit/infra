// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit;

import jakarta.persistence.*;
import java.time.Instant;

/**
 * JPA entity mapping to {@code audit.audit_log} — the append-only audit trail.
 *
 * <p>Rows are immutable once persisted (enforced by DB triggers).
 * Use the fluent {@link AuditEventBuilder} to construct instances.
 */
@Entity
@Table(name = "audit_log", schema = "audit")
public class AuditEvent {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "event_time", nullable = false)
    private Instant eventTime = Instant.now();

    @Column(name = "actor_id")
    private String actorId;

    @Column(name = "actor_type", nullable = false)
    @Enumerated(EnumType.STRING)
    private ActorType actorType;

    @Column(nullable = false)
    private String action;

    @Column(name = "resource_type", nullable = false)
    private String resourceType;

    @Column(name = "resource_id")
    private String resourceId;

    @Column(name = "ip_address")
    private String ipAddress;

    @Column(name = "user_agent")
    private String userAgent;

    @Column(nullable = false)
    @Enumerated(EnumType.STRING)
    private AuditResult result;

    @Column(columnDefinition = "jsonb")
    private String metadata;

    @Column(name = "trace_id")
    private String traceId;

    @Column(name = "service_name", nullable = false)
    private String serviceName;

    // ── Enums ─────────────────────────────────────────────────────────

    public enum ActorType { USER, SERVICE, SYSTEM, ANONYMOUS }

    public enum AuditResult { SUCCESS, FAILURE, DENIED }

    // ── Getters ───────────────────────────────────────────────────────

    public Long getId() { return id; }
    public Instant getEventTime() { return eventTime; }
    public String getActorId() { return actorId; }
    public ActorType getActorType() { return actorType; }
    public String getAction() { return action; }
    public String getResourceType() { return resourceType; }
    public String getResourceId() { return resourceId; }
    public String getIpAddress() { return ipAddress; }
    public String getUserAgent() { return userAgent; }
    public AuditResult getResult() { return result; }
    public String getMetadata() { return metadata; }
    public String getTraceId() { return traceId; }
    public String getServiceName() { return serviceName; }

    // ── Builder ───────────────────────────────────────────────────────

    public static AuditEventBuilder builder() { return new AuditEventBuilder(); }

    public static class AuditEventBuilder {
        private final AuditEvent event = new AuditEvent();

        public AuditEventBuilder actorId(String v) { event.actorId = v; return this; }
        public AuditEventBuilder actorType(ActorType v) { event.actorType = v; return this; }
        public AuditEventBuilder action(String v) { event.action = v; return this; }
        public AuditEventBuilder resourceType(String v) { event.resourceType = v; return this; }
        public AuditEventBuilder resourceId(String v) { event.resourceId = v; return this; }
        public AuditEventBuilder ipAddress(String v) { event.ipAddress = v; return this; }
        public AuditEventBuilder userAgent(String v) { event.userAgent = v; return this; }
        public AuditEventBuilder result(AuditResult v) { event.result = v; return this; }
        public AuditEventBuilder metadata(String v) { event.metadata = v; return this; }
        public AuditEventBuilder traceId(String v) { event.traceId = v; return this; }
        public AuditEventBuilder serviceName(String v) { event.serviceName = v; return this; }

        public AuditEvent build() { return event; }
    }
}
