package bf.gov.faso.auth.config;

import bf.gov.faso.auth.infra.security.StepUpAuthFilter;
import bf.gov.faso.auth.security.JwtAuthenticationFilter;
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
 * Spring Security configuration for auth-ms.
 * <p>
 * This is a management-plane service -- ARMAGEDDON handles production
 * session validation via jwt_authn. Here we enforce JWT auth for admin
 * GraphQL operations and permit the JWKS endpoint without auth.
 */
@Configuration
@EnableWebSecurity
@EnableMethodSecurity
public class SecurityConfig {

    private final JwtAuthenticationFilter jwtAuthenticationFilter;
    private final StepUpAuthFilter stepUpAuthFilter;

    public SecurityConfig(JwtAuthenticationFilter jwtAuthenticationFilter,
                          StepUpAuthFilter stepUpAuthFilter) {
        this.jwtAuthenticationFilter = jwtAuthenticationFilter;
        this.stepUpAuthFilter = stepUpAuthFilter;
    }

    @Bean
    public SecurityFilterChain securityFilterChain(HttpSecurity http) throws Exception {
        http
            .csrf(AbstractHttpConfigurer::disable)
            .sessionManagement(session -> session
                .sessionCreationPolicy(SessionCreationPolicy.STATELESS))
            .authorizeHttpRequests(auth -> auth
                // JWKS endpoint: public (consumed by ARMAGEDDON)
                .requestMatchers("/.well-known/jwks.json").permitAll()
                // Safe actuator probes + Prometheus metrics scraping: public.
                // K8s liveness/readiness MUST be unauthenticated so kubelet
                // (and ARMAGEDDON gateway health checks) can probe without
                // a JWT. Returns 200 only when the dedicated liveness/readiness
                // health groups (db,redis,ping,livenessState/readinessState)
                // are all UP — see management.endpoint.health.group.* config.
                .requestMatchers("/actuator/health", "/actuator/info",
                                 "/actuator/health/liveness", "/actuator/health/readiness",
                                 "/actuator/prometheus", "/actuator/metrics/**").permitAll()
                // Sensitive actuator endpoints: require ACTUATOR role
                .requestMatchers("/actuator/**").hasRole("ACTUATOR")
                // GraphiQL UI: never exposed via HTTP (OWASP A05).
                // Even when spring.graphql.graphiql.enabled is false the
                // route is still 200-able if .permitAll() is set; deny it
                // explicitly here.
                .requestMatchers("/graphiql/**").denyAll()
                // Delta amendment 2026-04-30: account recovery + login-time
                // recovery code consumption are pre-auth flows.
                // Phase 4.b.4: magic-link onboarding & recovery verify-* are
                // pre-auth (no JWT yet — they bootstrap the user's session).
                .requestMatchers(
                        "/admin/auth/recovery/initiate",
                        "/admin/auth/recovery/complete",
                        "/admin/auth/recovery/verify-link",
                        "/admin/auth/recovery/verify-otp",
                        "/admin/auth/login/recovery-code",
                        // Phase 4.b.6 — risk scoring runs after Kratos password
                        // verify but before the MFA challenge (still pre-auth).
                        "/admin/auth/login/risk",
                        "/admin/auth/onboard/begin",
                        "/admin/auth/onboard/verify-link",
                        "/admin/auth/onboard/verify-otp"
                ).permitAll()
                // Everything else requires authentication
                .anyRequest().authenticated()
            )
            .addFilterBefore(jwtAuthenticationFilter, UsernamePasswordAuthenticationFilter.class)
            // Phase 4.b.7 — runs AFTER jwt auth (so SecurityContext principal is
            // populated) but BEFORE the controller dispatch.
            .addFilterAfter(stepUpAuthFilter, JwtAuthenticationFilter.class);

        return http.build();
    }
}
