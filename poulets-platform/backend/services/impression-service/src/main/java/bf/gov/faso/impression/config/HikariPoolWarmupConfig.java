package bf.gov.faso.impression.config;

import com.zaxxer.hikari.HikariDataSource;
import lombok.extern.slf4j.Slf4j;
import org.springframework.boot.context.event.ApplicationReadyEvent;
import org.springframework.context.event.EventListener;
import org.springframework.stereotype.Component;
import javax.sql.DataSource;

@Component
@Slf4j
public class HikariPoolWarmupConfig {
    private final DataSource dataSource;
    public HikariPoolWarmupConfig(DataSource dataSource) { this.dataSource = dataSource; }

    @EventListener(ApplicationReadyEvent.class)
    public void warmupConnectionPool() {
        if (!(dataSource instanceof HikariDataSource hds)) return;
        try (var conn = hds.getConnection(); var stmt = conn.createStatement()) {
            stmt.execute("SELECT 1");
            var pool = hds.getHikariPoolMXBean();
            log.info("HikariCP pool '{}' warmed up — active={}, idle={}, total={}",
                    hds.getPoolName(), pool.getActiveConnections(),
                    pool.getIdleConnections(), pool.getTotalConnections());
        } catch (Exception e) { log.warn("Failed to warm up HikariCP pool: {}", e.getMessage()); }
    }
}
