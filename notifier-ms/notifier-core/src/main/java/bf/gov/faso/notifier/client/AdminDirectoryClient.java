/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.client;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Component;
import org.springframework.web.client.RestClient;

import java.time.Duration;
import java.util.List;

/**
 * AdminDirectoryClient — REST client to list active SUPER-ADMIN email addresses
 * via {@code auth-ms} ({@code GET /admin/users?role=SUPER_ADMIN&active=true}).
 *
 * <p>Result is cached in KAYA for 60 s to throttle inter-service calls.
 */
@Component
public class AdminDirectoryClient {

    private static final Logger log = LoggerFactory.getLogger(AdminDirectoryClient.class);
    private static final Duration CACHE_TTL = Duration.ofSeconds(60);
    private static final String CACHE_KEY = "notifier:super-admins:emails";

    private final RestClient restClient;
    private final StringRedisTemplate kayaTemplate;
    private final ObjectMapper objectMapper;

    public AdminDirectoryClient(
            @Value("${notifier.auth-ms.base-url:http://auth-ms:8801}") String authMsBaseUrl,
            StringRedisTemplate kayaTemplate,
            ObjectMapper objectMapper) {
        this.restClient = RestClient.builder().baseUrl(authMsBaseUrl).build();
        this.kayaTemplate = kayaTemplate;
        this.objectMapper = objectMapper;
    }

    public List<String> listSuperAdminEmails() {
        try {
            String cached = kayaTemplate.opsForValue().get(CACHE_KEY);
            if (cached != null) {
                return objectMapper.readValue(cached, new TypeReference<List<String>>() {});
            }
        } catch (Exception ex) {
            log.debug("KAYA SA cache read failed: {}", ex.getMessage());
        }

        try {
            String body = restClient.get()
                .uri(uri -> uri.path("/admin/users")
                    .queryParam("role", "SUPER_ADMIN")
                    .queryParam("active", "true")
                    .queryParam("fields", "email")
                    .build())
                .retrieve()
                .body(String.class);
            if (body == null || body.isBlank()) return List.of();

            List<String> emails = objectMapper.readValue(body, new TypeReference<List<String>>() {});
            try {
                kayaTemplate.opsForValue().set(CACHE_KEY, objectMapper.writeValueAsString(emails), CACHE_TTL);
            } catch (Exception ex) {
                log.debug("KAYA SA cache write failed: {}", ex.getMessage());
            }
            return emails;
        } catch (Exception e) {
            log.warn("Failed to list SUPER-ADMIN emails from auth-ms: {}", e.getMessage());
            return List.of();
        }
    }
}
