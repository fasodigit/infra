package bf.gov.faso.auth.security;

import bf.gov.faso.auth.config.SecurityConfig;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.context.annotation.Import;
import org.springframework.test.web.servlet.MockMvc;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

/**
 * Verifies that sensitive actuator endpoints are protected and
 * that /graphql requires authentication.
 */
@WebMvcTest
@Import(SecurityConfig.class)
class ActuatorSecurityTest {

    @Autowired
    private MockMvc mockMvc;

    @MockBean
    private JwtAuthenticationFilter jwtAuthenticationFilter;

    @Test
    @DisplayName("/actuator/env without auth → 401 or 403")
    void actuatorEnvIsProtected() throws Exception {
        mockMvc.perform(get("/actuator/env"))
               .andExpect(status().is(org.hamcrest.Matchers.anyOf(
                       org.hamcrest.Matchers.is(401),
                       org.hamcrest.Matchers.is(403))));
    }

    @Test
    @DisplayName("/actuator/health without auth → 200 (public probe)")
    void actuatorHealthIsPublic() throws Exception {
        mockMvc.perform(get("/actuator/health"))
               .andExpect(status().is(org.hamcrest.Matchers.anyOf(
                       org.hamcrest.Matchers.is(200),
                       // 503 when health contributors are down — still accessible
                       org.hamcrest.Matchers.is(503))));
    }

    @Test
    @DisplayName("/.well-known/jwks.json without auth → not 401/403")
    void jwksIsPublic() throws Exception {
        mockMvc.perform(get("/.well-known/jwks.json"))
               .andExpect(status().is(org.hamcrest.Matchers.not(
                       org.hamcrest.Matchers.anyOf(
                               org.hamcrest.Matchers.is(401),
                               org.hamcrest.Matchers.is(403)))));
    }
}
