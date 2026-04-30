package bf.gov.faso.poulets.config;

import bf.gov.faso.poulets.security.JwtAuthenticationFilter;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.security.config.annotation.method.configuration.EnableMethodSecurity;
import org.springframework.security.config.annotation.web.builders.HttpSecurity;
import org.springframework.security.config.annotation.web.configuration.EnableWebSecurity;
import org.springframework.security.config.annotation.web.configurers.AbstractHttpConfigurer;
import org.springframework.security.config.http.SessionCreationPolicy;
import org.springframework.security.web.SecurityFilterChain;
import org.springframework.security.web.authentication.UsernamePasswordAuthenticationFilter;

/**
 * Security configuration for poulets-api.
 * <p>
 * JWT tokens are issued by auth-ms and validated here via JWKS.
 * Method-level security ({@code @PreAuthorize}) guards mutations.
 */
@Configuration
@EnableWebSecurity
@EnableMethodSecurity(prePostEnabled = true, securedEnabled = true)
public class SecurityConfig {

    private final JwtAuthenticationFilter jwtAuthenticationFilter;

    public SecurityConfig(JwtAuthenticationFilter jwtAuthenticationFilter) {
        this.jwtAuthenticationFilter = jwtAuthenticationFilter;
    }

    @Bean
    public SecurityFilterChain securityFilterChain(HttpSecurity http) throws Exception {
        http
            .csrf(AbstractHttpConfigurer::disable)
            .sessionManagement(session -> session
                .sessionCreationPolicy(SessionCreationPolicy.STATELESS))
            .authorizeHttpRequests(auth -> auth
                // Public dashboard API (no auth required)
                .requestMatchers("/api/public/**").permitAll()
                // GraphiQL UI: never exposed via HTTP (OWASP A05).
                // Enabling spring.graphql.graphiql is not enough — the
                // route must also be denied at the security layer.
                .requestMatchers("/graphiql/**").denyAll()
                // Safe actuator probes + Prometheus metrics scraping: public.
                // K8s liveness/readiness must be unauthenticated for kubelet
                // and ARMAGEDDON gateway health checks.
                .requestMatchers("/actuator/health", "/actuator/info",
                                 "/actuator/health/liveness", "/actuator/health/readiness",
                                 "/actuator/prometheus", "/actuator/metrics/**").permitAll()
                // Sensitive actuator endpoints: require ACTUATOR role
                .requestMatchers("/actuator/**").hasRole("ACTUATOR")
                // All other endpoints (including /graphql) require authentication
                .anyRequest().authenticated()
            )
            .addFilterBefore(jwtAuthenticationFilter, UsernamePasswordAuthenticationFilter.class);

        return http.build();
    }
}
