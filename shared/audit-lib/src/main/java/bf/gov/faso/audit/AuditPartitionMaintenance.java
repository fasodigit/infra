// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit;

import jakarta.persistence.EntityManager;
import jakarta.persistence.PersistenceContext;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Component;
import org.springframework.transaction.annotation.Transactional;

import java.time.LocalDate;
import java.time.temporal.TemporalAdjusters;

/**
 * Sliding-window maintenance for {@code audit.audit_log} monthly partitions.
 *
 * <p>The Flyway migration creates 13 partitions at install time (current
 * month + 12 ahead). This component runs daily and ensures the window stays
 * 12 months ahead by creating the partition for {@code today + 12 months}.
 *
 * <p>If the partition already exists the helper function returns its name as
 * a no-op — the call is idempotent and cheap (single CTE).
 *
 * <p>Disable via {@code faso.audit.partition-maintenance.enabled=false} for
 * services that want to opt out (e.g. notifier-ms in shared-DB topology).
 */
@Component
@ConditionalOnProperty(prefix = "faso.audit.partition-maintenance", name = "enabled", havingValue = "true", matchIfMissing = true)
public class AuditPartitionMaintenance {

    private static final Logger log = LoggerFactory.getLogger(AuditPartitionMaintenance.class);

    @PersistenceContext
    private EntityManager em;

    /**
     * Runs every day at 02:30 (server local time, default UTC). Creates the
     * partition for the month 12 months ahead of "today" so the sliding
     * window stays 12 months wide.
     */
    @Scheduled(cron = "${faso.audit.partition-maintenance.cron:0 30 2 * * *}")
    @Transactional
    public void rollForward() {
        LocalDate target = LocalDate.now()
                .with(TemporalAdjusters.firstDayOfMonth())
                .plusMonths(12);
        try {
            String name = (String) em.createNativeQuery(
                            "SELECT audit.ensure_audit_log_partition(CAST(:d AS DATE))")
                    .setParameter("d", java.sql.Date.valueOf(target))
                    .getSingleResult();
            log.info("audit_log partition ensured for {} → {}", target, name);
        } catch (Exception e) {
            log.error("Failed to ensure audit_log partition for {}: {}",
                    target, e.getMessage(), e);
        }
    }
}
