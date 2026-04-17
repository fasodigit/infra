package bf.gov.faso.poulets.security;

import bf.gov.faso.poulets.config.SecurityConfig;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.context.annotation.Import;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

/**
 * Verifies that /graphql POST and sensitive actuator endpoints require authentication.
 */
@WebMvcTest
@Import(SecurityConfig.class)
class SecurityEndpointTest {

    @Autowired
    private MockMvc mockMvc;

    @MockBean
    private JwtAuthenticationFilter jwtAuthenticationFilter;

    @Test
    @DisplayName("POST /graphql without auth → 401")
    void graphqlRequiresAuth() throws Exception {
        mockMvc.perform(post("/graphql")
                .contentType(MediaType.APPLICATION_JSON)
                .content("{\"query\":\"{ poulets { content { id } } }\"}"))
               .andExpect(status().isUnauthorized());
    }

    @Test
    @DisplayName("/actuator/env without auth → 401 or 403")
    void actuatorEnvIsProtected() throws Exception {
        mockMvc.perform(get("/actuator/env"))
               .andExpect(status().is(org.hamcrest.Matchers.anyOf(
                       org.hamcrest.Matchers.is(401),
                       org.hamcrest.Matchers.is(403))));
    }

    @Test
    @DisplayName("/actuator/health without auth → accessible (200 or 503)")
    void actuatorHealthIsPublic() throws Exception {
        mockMvc.perform(get("/actuator/health"))
               .andExpect(status().is(org.hamcrest.Matchers.anyOf(
                       org.hamcrest.Matchers.is(200),
                       org.hamcrest.Matchers.is(503))));
    }

    @Test
    @DisplayName("/api/public/** without auth → not 401/403")
    void publicDashboardIsAccessible() throws Exception {
        mockMvc.perform(get("/api/public/stats"))
               .andExpect(status().is(org.hamcrest.Matchers.not(
                       org.hamcrest.Matchers.anyOf(
                               org.hamcrest.Matchers.is(401),
                               org.hamcrest.Matchers.is(403)))));
    }
}
