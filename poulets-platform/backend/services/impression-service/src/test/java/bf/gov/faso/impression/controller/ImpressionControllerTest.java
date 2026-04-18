package bf.gov.faso.impression.controller;

import bf.gov.faso.impression.dto.request.AddToQueueRequest;
import bf.gov.faso.impression.dto.response.PageResponse;
import bf.gov.faso.impression.dto.response.PrintJobResponse;
import bf.gov.faso.impression.entity.PrintStatus;
import bf.gov.faso.impression.security.JwtUser;
import bf.gov.faso.impression.service.BlockchainService;
import bf.gov.faso.impression.service.ImpressionService;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.http.MediaType;
import org.springframework.security.test.context.support.WithMockUser;
import org.springframework.test.web.servlet.MockMvc;

import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.UUID;

import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.when;
import static org.springframework.security.test.web.servlet.request.SecurityMockMvcRequestPostProcessors.jwt;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.*;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@WebMvcTest(ImpressionController.class)
class ImpressionControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private ObjectMapper objectMapper;

    @MockBean
    private ImpressionService impressionService;

    @MockBean
    private BlockchainService blockchainService;

    private UUID testPrintJobId;
    private UUID testDocumentId;
    private UUID testDemandeId;
    private UUID testClientId;
    private String testTenantId;
    private PrintJobResponse testPrintJobResponse;

    @BeforeEach
    void setUp() {
        testPrintJobId = UUID.randomUUID();
        testDocumentId = UUID.randomUUID();
        testDemandeId = UUID.randomUUID();
        testClientId = UUID.randomUUID();
        testTenantId = "test-tenant";

        testPrintJobResponse = new PrintJobResponse(
            testPrintJobId,
            testDocumentId,
            testDemandeId,
            testTenantId,
            testClientId,
            PrintStatus.EN_ATTENTE,
            5,
            "ACTE_NAISSANCE",
            "NAI-2025-000001",
            null,
            null,
            null,
            null,
            null,
            1,
            0,
            false,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            0,
            null,
            null,
            null,
            Map.of(),
            null,
            null,
            Instant.now(),
            Instant.now()
        );
    }

    @Test
    @DisplayName("Should get print queue with OPERATEUR_IMPRESSION role")
    void shouldGetPrintQueueWithOperateurRole() throws Exception {
        // Given
        PageResponse<PrintJobResponse> response = new PageResponse<>(
            List.of(testPrintJobResponse),
            0, 20, 1, 1, true, true, false, false
        );

        when(impressionService.getQueue(anyString(), anyInt(), anyInt()))
            .thenReturn(response);

        // When & Then
        mockMvc.perform(get("/api/v1/impression/queue")
                .with(jwt().jwt(jwt -> jwt
                    .subject(UUID.randomUUID().toString())
                    .claim("tenant_id", testTenantId)
                    .claim("roles", List.of("OPERATEUR_IMPRESSION")))))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.content").isArray())
            .andExpect(jsonPath("$.content[0].id").exists());
    }

    @Test
    @DisplayName("Should add document to queue")
    void shouldAddDocumentToQueue() throws Exception {
        // Given
        AddToQueueRequest request = new AddToQueueRequest(
            testDocumentId,
            testDemandeId,
            testClientId,
            "ACTE_NAISSANCE",
            "NAI-2025-000001",
            5,
            1,
            "/storage/documents/test.pdf",
            null,
            null,
            null,
            null
        );

        when(impressionService.addToQueue(any(), anyString()))
            .thenReturn(testPrintJobResponse);

        // When & Then
        mockMvc.perform(post("/api/v1/impression/queue")
                .with(jwt().jwt(jwt -> jwt
                    .subject(UUID.randomUUID().toString())
                    .claim("tenant_id", testTenantId)
                    .claim("roles", List.of("OPERATEUR_IMPRESSION"))))
                .contentType(MediaType.APPLICATION_JSON)
                .content(objectMapper.writeValueAsString(request)))
            .andExpect(status().isCreated())
            .andExpect(jsonPath("$.id").exists())
            .andExpect(jsonPath("$.status").value("EN_ATTENTE"));
    }

    @Test
    @DisplayName("Should print document")
    void shouldPrintDocument() throws Exception {
        // Given
        PrintJobResponse printedResponse = new PrintJobResponse(
            testPrintJobId,
            testDocumentId,
            testDemandeId,
            testTenantId,
            testClientId,
            PrintStatus.IMPRIME,
            5,
            "ACTE_NAISSANCE",
            "NAI-2025-000001",
            UUID.randomUUID(),
            Instant.now(),
            null,
            null,
            null,
            1,
            1,
            true,
            "validated-test-tenant",
            "documents/" + testDocumentId + ".pdf",
            Instant.now(),
            Instant.now().plusSeconds(3650L * 24 * 60 * 60),
            "hash123",
            "blockhash456",
            null,
            0,
            null,
            null,
            null,
            Map.of(),
            null,
            null,
            Instant.now(),
            Instant.now()
        );

        when(impressionService.printDocument(any(), any(), anyString()))
            .thenReturn(printedResponse);

        // When & Then
        mockMvc.perform(post("/api/v1/impression/{printJobId}/print", testPrintJobId)
                .with(jwt().jwt(jwt -> jwt
                    .subject(UUID.randomUUID().toString())
                    .claim("tenant_id", testTenantId)
                    .claim("roles", List.of("OPERATEUR_IMPRESSION")))))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.status").value("IMPRIME"))
            .andExpect(jsonPath("$.wormLocked").value(true));
    }

    @Test
    @DisplayName("Should get print job status")
    void shouldGetPrintJobStatus() throws Exception {
        // Given
        when(impressionService.getPrintStatus(any(), anyString()))
            .thenReturn(PrintStatus.IMPRIME);

        // When & Then
        mockMvc.perform(get("/api/v1/impression/{printJobId}/status", testPrintJobId)
                .with(jwt().jwt(jwt -> jwt
                    .subject(UUID.randomUUID().toString())
                    .claim("tenant_id", testTenantId)
                    .claim("roles", List.of("OPERATEUR_IMPRESSION")))))
            .andExpect(status().isOk())
            .andExpect(content().string("\"IMPRIME\""));
    }

    @Test
    @DisplayName("Should deny access without proper role")
    void shouldDenyAccessWithoutProperRole() throws Exception {
        // When & Then
        mockMvc.perform(get("/api/v1/impression/queue")
                .with(jwt().jwt(jwt -> jwt
                    .subject(UUID.randomUUID().toString())
                    .claim("tenant_id", testTenantId)
                    .claim("roles", List.of("OPERATEUR_TRAITEMENT"))))) // Wrong role
            .andExpect(status().isForbidden());
    }

    @Test
    @DisplayName("Should require authentication")
    void shouldRequireAuthentication() throws Exception {
        mockMvc.perform(get("/api/v1/impression/queue"))
            .andExpect(status().isUnauthorized());
    }
}
