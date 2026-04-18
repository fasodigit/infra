package bf.gov.faso.impression.config;

import bf.gov.faso.impression.security.JwtUser;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.context.annotation.Profile;
import org.springframework.core.convert.converter.Converter;
import org.springframework.security.authentication.AbstractAuthenticationToken;
import org.springframework.security.config.annotation.web.builders.HttpSecurity;
import org.springframework.security.config.annotation.web.configuration.EnableWebSecurity;
import org.springframework.security.config.annotation.web.configurers.AbstractHttpConfigurer;
import org.springframework.security.config.http.SessionCreationPolicy;
import org.springframework.security.core.GrantedAuthority;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.security.oauth2.server.resource.authentication.JwtAuthenticationToken;
import org.springframework.security.web.SecurityFilterChain;
import org.springframework.security.web.authentication.UsernamePasswordAuthenticationFilter;

import java.util.Collection;

/**
 * Security configuration for the impression service.
 *
 * Uses OAuth2 Resource Server with JWT validation.
 * Role-based access control enforced via method security.
 */
@Configuration
@EnableWebSecurity
public class SecurityConfig {

    /**
     * Dev/local profile: permit all requests and inject fake user for testing.
     */
    @Bean
    @Profile({"dev", "local"})
    public SecurityFilterChain devSecurityFilterChain(HttpSecurity http,
            DevAuthenticationFilter devAuthFilter) throws Exception {
        http
            .csrf(AbstractHttpConfigurer::disable)
            .sessionManagement(session -> session
                .sessionCreationPolicy(SessionCreationPolicy.STATELESS))
            .authorizeHttpRequests(auth -> auth
                .anyRequest().permitAll())
            .addFilterBefore(devAuthFilter, UsernamePasswordAuthenticationFilter.class);

        return http.build();
    }

    /**
     * Production profile: full JWT authentication.
     */
    @Bean
    @Profile("prod")
    public SecurityFilterChain securityFilterChain(HttpSecurity http) throws Exception {
        http
            .csrf(csrf -> csrf.disable())
            .sessionManagement(session -> session
                .sessionCreationPolicy(SessionCreationPolicy.STATELESS))
            .authorizeHttpRequests(authorize -> authorize
                // Public endpoints
                .requestMatchers(
                    "/actuator/health",
                    "/actuator/info",
                    "/actuator/prometheus",
                    "/v3/api-docs/**",
                    "/swagger-ui/**",
                    "/swagger-ui.html",
                    "/api/v1/verification/**"
                ).permitAll()
                // All other endpoints require authentication
                .anyRequest().authenticated()
            )
            .oauth2ResourceServer(oauth2 -> oauth2
                .jwt(jwt -> jwt.jwtAuthenticationConverter(jwtAuthenticationConverter()))
            );

        return http.build();
    }

    @Bean
    public Converter<Jwt, AbstractAuthenticationToken> jwtAuthenticationConverter() {
        return new Converter<>() {
            @Override
            public AbstractAuthenticationToken convert(Jwt jwt) {
                JwtUser jwtUser = new JwtUser(jwt);
                Collection<GrantedAuthority> authorities = jwtUser.getAuthorities();
                return new JwtAuthenticationToken(jwt, authorities) {
                    @Override
                    public Object getPrincipal() {
                        return jwtUser;
                    }
                };
            }
        };
    }
}
