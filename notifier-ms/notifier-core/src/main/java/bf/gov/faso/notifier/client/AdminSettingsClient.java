/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.client;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.http.HttpStatusCode;
import org.springframework.stereotype.Component;
import org.springframework.web.client.RestClient;

import java.time.Duration;

/**
 * AdminSettingsClient — REST client to fetch admin settings from {@code auth-ms}
 * (e.g. {@code mfa.notify_user_on_session_revoke}). Values are cached in KAYA
 * for 30 seconds to limit chatty inter-service calls.
 *
 * <p>Endpoint contract: {@code GET {auth-ms}/admin/settings/{key}} → 200
 * {@code {"key":"...","value":"..."}} or 404 if absent. Failure to reach auth-ms
 * is treated as "value unknown" — callers must apply a safe default.
 */
@Component
public class AdminSettingsClient {

    private static final Logger log = LoggerFactory.getLogger(AdminSettingsClient.class);
    private static final Duration CACHE_TTL = Duration.ofSeconds(30);
    private static final String CACHE_PREFIX = "notifier:admin-settings:";

    private final RestClient restClient;
    private final StringRedisTemplate kayaTemplate;
    private final ObjectMapper objectMapper;

    public AdminSettingsClient(
            @Value("${notifier.auth-ms.base-url:http://auth-ms:8801}") String authMsBaseUrl,
            StringRedisTemplate kayaTemplate,
            ObjectMapper objectMapper) {
        this.restClient = RestClient.builder().baseUrl(authMsBaseUrl).build();
        this.kayaTemplate = kayaTemplate;
        this.objectMapper = objectMapper;
    }

    /**
     * Fetch a string setting; returns {@code defaultValue} on miss/error.
     */
    public String getString(String key, String defaultValue) {
        String cacheKey = CACHE_PREFIX + key;
        try {
            String cached = kayaTemplate.opsForValue().get(cacheKey);
            if (cached != null) {
                return cached;
            }
        } catch (Exception ex) {
            log.debug("KAYA cache miss/unavailable for {}: {}", key, ex.getMessage());
        }

        try {
            String body = restClient.get()
                .uri("/admin/settings/{key}", key)
                .retrieve()
                .onStatus(HttpStatusCode::is4xxClientError, (req, resp) -> { /* swallow */ })
                .body(String.class);
            if (body == null || body.isBlank()) {
                return defaultValue;
            }
            JsonNode node = objectMapper.readTree(body);
            String value = node.has("value") && !node.get("value").isNull()
                ? node.get("value").asText()
                : defaultValue;

            try {
                kayaTemplate.opsForValue().set(cacheKey, value, CACHE_TTL);
            } catch (Exception ex) {
                log.debug("KAYA cache write failed for {}: {}", key, ex.getMessage());
            }
            return value;
        } catch (Exception e) {
            log.warn("Failed to fetch admin setting '{}' from auth-ms: {} — using default '{}'",
                key, e.getMessage(), defaultValue);
            return defaultValue;
        }
    }

    /** Boolean variant using {@link #getString(String, String)}. */
    public boolean getBoolean(String key, boolean defaultValue) {
        String raw = getString(key, Boolean.toString(defaultValue));
        return Boolean.parseBoolean(raw);
    }
}
