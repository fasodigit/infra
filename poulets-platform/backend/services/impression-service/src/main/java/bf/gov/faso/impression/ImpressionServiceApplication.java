package bf.gov.faso.impression;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.cache.annotation.EnableCaching;
import org.springframework.data.jpa.repository.config.EnableJpaAuditing;
import org.springframework.scheduling.annotation.EnableAsync;
import org.springframework.scheduling.annotation.EnableScheduling;

/**
 * Main application class for Impression Service.
 *
 * This service manages document printing, WORM (Write Once Read Many) storage,
 * and blockchain audit trail for the multi-tenant civil registry platform.
 *
 * Features:
 * - WORM storage with MinIO Object Lock for document immutability
 * - Blockchain-style audit trail for traceability
 * - Print queue management with priority support
 * - PDF generation from templates with watermarking
 * - Role-based access control (OPERATEUR_IMPRESSION only)
 * - Multi-tenant isolation via schema-per-tenant
 * - ZGC generational garbage collector for low latency
 */
@SpringBootApplication
@EnableCaching
@EnableJpaAuditing
@EnableAsync
@EnableScheduling
public class ImpressionServiceApplication {

    public static void main(String[] args) {
        SpringApplication.run(ImpressionServiceApplication.class, args);
    }
}
