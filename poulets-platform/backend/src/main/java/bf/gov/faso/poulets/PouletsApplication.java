package bf.gov.faso.poulets;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.scheduling.annotation.EnableScheduling;

/**
 * Poulets API - Chicken selling platform for FASO DIGITALISATION.
 * <p>
 * Connects eleveurs (farmers) with clients (buyers) for chicken commerce.
 * <p>
 * Nord-Sud (external): GraphQL (Netflix DGS) on port 8901
 * Est-Ouest (internal): gRPC on port 9901
 * Cache: KAYA (Redis-compatible) on port 6380
 */
@SpringBootApplication
@EnableScheduling
public class PouletsApplication {

    public static void main(String[] args) {
        SpringApplication.run(PouletsApplication.class, args);
    }
}
