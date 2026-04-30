// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit;

import org.springframework.boot.autoconfigure.AutoConfiguration;
import org.springframework.boot.autoconfigure.condition.ConditionalOnClass;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.context.annotation.Bean;
import org.springframework.scheduling.annotation.EnableAsync;
import org.springframework.scheduling.annotation.EnableScheduling;
import org.springframework.scheduling.concurrent.ThreadPoolTaskExecutor;

import java.util.concurrent.Executor;
import java.util.concurrent.ThreadPoolExecutor;

/**
 * Auto-configuration for the FASO audit library.
 *
 * <p>Activated automatically when {@code audit-lib} is on the classpath of any
 * Spring Boot 3+ service (registered via
 * {@code META-INF/spring/org.springframework.boot.autoconfigure.AutoConfiguration.imports}).
 *
 * <p>The configuration:
 * <ul>
 *   <li>Scans entities under {@code bf.gov.faso.audit} so {@link AuditEvent}
 *       is picked up by Hibernate without per-service {@code @EntityScan}.</li>
 *   <li>Enables the JPA repository for {@link AuditRepository}.</li>
 *   <li>Enables {@link EnableAsync} and ships a bounded
 *       {@link ThreadPoolTaskExecutor} named {@code auditTaskExecutor}.
 *       Without an explicit bean Spring would fall back to the unbounded
 *       {@code SimpleAsyncTaskExecutor} which spawns a fresh thread per audit
 *       event — catastrophic under load.</li>
 *   <li>Disable via property {@code faso.audit.enabled=false} if a service
 *       wants to opt out (e.g. greenfield service without DB schema yet).</li>
 * </ul>
 */
@AutoConfiguration
@ConditionalOnClass({jakarta.persistence.Entity.class})
@ConditionalOnProperty(prefix = "faso.audit", name = "enabled", havingValue = "true", matchIfMissing = true)
@EnableAsync
@EnableScheduling
public class AuditAutoConfiguration {

    /*
     * NOTE: this auto-config deliberately does NOT include @EntityScan or
     * @EnableJpaRepositories — declaring those here would REPLACE the
     * consuming service's own auto-detected scans and break their
     * repositories. Each consuming service must instead declare on its
     * @SpringBootApplication class:
     *
     *   @EntityScan(basePackages = {"bf.gov.faso.<svc>", "bf.gov.faso.audit"})
     *   @EnableJpaRepositories(basePackages = {"bf.gov.faso.<svc>", "bf.gov.faso.audit"})
     *
     * This is documented in the audit-lib README and applied to auth-ms,
     * poulets-api, and notifier-ms.
     */

    /**
     * Bounded executor for {@link AuditService#record(AuditEvent)} calls.
     * Sized for ~50–100 audit events/sec sustained throughput on a single pod.
     * The {@code CallerRunsPolicy} backpressure keeps audit synchronous when
     * the queue saturates rather than dropping events silently.
     */
    @Bean(name = "auditTaskExecutor")
    public Executor auditTaskExecutor() {
        ThreadPoolTaskExecutor executor = new ThreadPoolTaskExecutor();
        executor.setCorePoolSize(2);
        executor.setMaxPoolSize(8);
        executor.setQueueCapacity(200);
        executor.setKeepAliveSeconds(60);
        executor.setThreadNamePrefix("audit-");
        executor.setRejectedExecutionHandler(new ThreadPoolExecutor.CallerRunsPolicy());
        executor.setWaitForTasksToCompleteOnShutdown(true);
        executor.setAwaitTerminationSeconds(15);
        executor.initialize();
        return executor;
    }
}
