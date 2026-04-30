// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.persistence.EntityManager;
import jakarta.persistence.PersistenceContext;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.slf4j.MDC;
import org.springframework.scheduling.annotation.Async;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Propagation;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Centralised audit-log writer for the admin plane.
 * <p>
 * Writes a row to {@code audit_log} (immutable per V9 trigger when
 * {@code admin_settings.audit.immutable_mode = true}) and asynchronously
 * publishes to the Redpanda audit topic. Publishing is best-effort — a
 * failure is logged but does NOT roll back the DB transaction.
 */
@Service
public class AdminAuditService {

    private static final Logger log = LoggerFactory.getLogger(AdminAuditService.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @PersistenceContext
    private EntityManager em;

    private final AdminEventProducer eventProducer;

    public AdminAuditService(AdminEventProducer eventProducer) {
        this.eventProducer = eventProducer;
    }

    @Transactional(propagation = Propagation.REQUIRES_NEW)
    public void log(String action, UUID actorId, String targetRef,
                    Object oldValue, Object newValue, String ipAddress) {
        try {
            String oldJson = oldValue == null ? null : MAPPER.writeValueAsString(oldValue);
            String newJson = newValue == null ? null : MAPPER.writeValueAsString(newValue);
            String traceId = MDC.get("traceId");

            String[] target = splitTarget(targetRef);
            String resourceType = target[0];
            String resourceId = target[1];

            String sql = "INSERT INTO audit_log (actor_id, action, target_type, target_id, " +
                    "details, ip_address, resource_type, old_value, new_value, metadata, " +
                    "trace_id, user_agent, created_at) " +
                    "VALUES (:actorId, :action, :targetType, :targetId, " +
                    "CAST(:details AS jsonb), :ip, :resourceType, " +
                    "CAST(:oldValue AS jsonb), CAST(:newValue AS jsonb), " +
                    "CAST(:metadata AS jsonb), :traceId, :ua, :createdAt)";

            em.createNativeQuery(sql)
              .setParameter("actorId", actorId)
              .setParameter("action", action)
              .setParameter("targetType", resourceType)
              .setParameter("targetId", resourceId)
              .setParameter("details", newJson)
              .setParameter("ip", ipAddress)
              .setParameter("resourceType", resourceType)
              .setParameter("oldValue", oldJson)
              .setParameter("newValue", newJson)
              .setParameter("metadata", null)
              .setParameter("traceId", traceId)
              .setParameter("ua", null)
              .setParameter("createdAt", Instant.now())
              .executeUpdate();

            // Publish async (DLQ-on-fail handled inside the producer).
            publishAsync(action, actorId, targetRef, oldJson, newJson, traceId);
        } catch (JsonProcessingException e) {
            log.error("Failed to serialize audit values for action={}: {}", action, e.getMessage());
        } catch (Exception e) {
            log.error("Failed to write audit_log row for action={}: {}", action, e.getMessage());
        }
    }

    @Async
    void publishAsync(String action, UUID actorId, String targetRef,
                      String oldJson, String newJson, String traceId) {
        try {
            Map<String, Object> payload = new LinkedHashMap<>();
            payload.put("id", UUID.randomUUID().toString());
            payload.put("action", action);
            payload.put("actorId", actorId == null ? null : actorId.toString());
            payload.put("target", targetRef);
            payload.put("oldValue", oldJson);
            payload.put("newValue", newJson);
            payload.put("traceId", traceId);
            payload.put("at", Instant.now().toString());
            eventProducer.publishAuditEvent(payload);
        } catch (Exception e) {
            log.warn("Audit publish failed (will only persist in DB) action={}: {}",
                    action, e.getMessage());
        }
    }

    /**
     * Query audit rows. Limited filters in iteration 1; the BFF/UI is
     * expected to add pagination cursors via {@code created_at}.
     */
    public List<Map<String, Object>> query(String action, UUID actorId,
                                           Instant from, Instant to, int limit) {
        StringBuilder sql = new StringBuilder(
                "SELECT id, actor_id, action, target_type, target_id, details, " +
                "ip_address, resource_type, old_value, new_value, metadata, trace_id, " +
                "user_agent, created_at FROM audit_log WHERE 1=1");

        Map<String, Object> params = new LinkedHashMap<>();
        if (action != null) { sql.append(" AND action = :action"); params.put("action", action); }
        if (actorId != null) { sql.append(" AND actor_id = :actorId"); params.put("actorId", actorId); }
        if (from != null) { sql.append(" AND created_at >= :from"); params.put("from", from); }
        if (to != null) { sql.append(" AND created_at <= :to"); params.put("to", to); }
        sql.append(" ORDER BY created_at DESC LIMIT :limit");
        params.put("limit", Math.max(1, Math.min(limit, 1000)));

        var query = em.createNativeQuery(sql.toString());
        params.forEach(query::setParameter);

        @SuppressWarnings("unchecked")
        List<Object[]> rows = query.getResultList();
        return rows.stream().map(this::rowToMap).toList();
    }

    private Map<String, Object> rowToMap(Object[] row) {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("id", row[0]);
        m.put("actorId", row[1]);
        m.put("action", row[2]);
        m.put("targetType", row[3]);
        m.put("targetId", row[4]);
        m.put("details", row[5]);
        m.put("ipAddress", row[6]);
        m.put("resourceType", row[7]);
        m.put("oldValue", row[8]);
        m.put("newValue", row[9]);
        m.put("metadata", row[10]);
        m.put("traceId", row[11]);
        m.put("userAgent", row[12]);
        m.put("createdAt", row[13]);
        return m;
    }

    /**
     * CSV export — stub. TODO Phase 4.b iteration 2: stream rows in chunks
     * and emit to a {@code text/csv} response.
     */
    public String exportCsv(String action, UUID actorId, Instant from, Instant to) {
        // TODO Phase 4.b iteration 2 — server-streaming CSV with backpressure.
        return "id,actor_id,action,target_type,target_id,created_at\n";
    }

    private String[] splitTarget(String ref) {
        if (ref == null) return new String[]{null, null};
        int idx = ref.indexOf(':');
        if (idx < 0) return new String[]{ref, null};
        return new String[]{ref.substring(0, idx), ref.substring(idx + 1)};
    }
}
