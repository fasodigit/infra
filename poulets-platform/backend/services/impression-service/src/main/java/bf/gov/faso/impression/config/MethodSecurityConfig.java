package bf.gov.faso.impression.config;

import org.springframework.context.annotation.Configuration;
import org.springframework.context.annotation.Profile;
import org.springframework.security.config.annotation.method.configuration.EnableMethodSecurity;

/**
 * Method-level security (@PreAuthorize) only in production.
 * In dev/local mode, all endpoints are open for testing.
 */
@Configuration
@Profile("prod")
@EnableMethodSecurity(prePostEnabled = true)
public class MethodSecurityConfig {
}
