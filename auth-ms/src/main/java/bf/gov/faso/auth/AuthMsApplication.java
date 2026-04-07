package bf.gov.faso.auth;

import bf.gov.faso.auth.config.NativeHints;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.ImportRuntimeHints;
import org.springframework.scheduling.annotation.EnableScheduling;

/**
 * Authentication Microservice for FASO DIGITALISATION.
 * <p>
 * Management-plane only service responsible for:
 * - User/Role CRUD (admin interface via GraphQL/DGS)
 * - JWT ES384 key generation and rotation (90-day cycle)
 * - JWKS endpoint consumed by ARMAGEDDON gateway
 * - Keto synchronization (Zanzibar relation tuples)
 * - Session limiting (max 3 per user via KAYA)
 * - JWT blacklist (via KAYA)
 * - Anti brute-force graduated punishment
 * - Password expiration enforcement
 * <p>
 * Est-Ouest (internal): gRPC on port 9801
 * Nord-Sud (external):  GraphQL (Netflix DGS) on port 8801
 * Cache:                KAYA (Redis-compatible) on port 6380
 */
@SpringBootApplication
@EnableScheduling
@ImportRuntimeHints(NativeHints.class)
public class AuthMsApplication {

    public static void main(String[] args) {
        SpringApplication.run(AuthMsApplication.class, args);
    }
}
