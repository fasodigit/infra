// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
package bf.gov.faso.poulets.flags;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.InjectMocks;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.data.redis.core.ValueOperations;
import org.springframework.http.HttpEntity;
import org.springframework.http.HttpMethod;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.client.RestTemplate;

import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.*;

/**
 * Tests unitaires {@link FeatureFlagsService} avec Mockito.
 * Couvre : cache HIT, cache MISS → GrowthBook, parse + evaluate.
 */
@ExtendWith(MockitoExtension.class)
class FeatureFlagsServiceTest {

    @Mock private StringRedisTemplate redis;
    @Mock private ValueOperations<String, String> ops;
    @Mock private RestTemplate rest;

    private final ObjectMapper mapper = new ObjectMapper();

    @InjectMocks private FeatureFlagsService service;

    @BeforeEach
    void init() {
        service = new FeatureFlagsService(redis, rest, mapper);
        service.configure("http://gb:3100", "dev", "sdk-test");
        when(redis.opsForValue()).thenReturn(ops);
    }

    @Test
    void cacheHit_returnsEvaluatedFlag_withoutHttpCall() throws Exception {
        String cached = "{\"poulets.new-checkout\":{\"defaultValue\":true}}";
        when(ops.get(anyString())).thenReturn(cached);

        boolean on = service.isOn("poulets.new-checkout", Map.of("user_id", "u1"));

        assertThat(on).isTrue();
        verify(rest, never()).exchange(anyString(), any(HttpMethod.class), any(), eq(String.class));
    }

    @Test
    void cacheMiss_fetchesGrowthBook_andCaches() {
        when(ops.get(anyString())).thenReturn(null);
        String body = "{\"features\":{\"auth.webauthn-beta\":{\"defaultValue\":true}}}";
        when(rest.exchange(anyString(), eq(HttpMethod.GET), any(HttpEntity.class), eq(String.class)))
                .thenReturn(new ResponseEntity<>(body, HttpStatus.OK));

        boolean on = service.isOn("auth.webauthn-beta", Map.of("user_id", "u7"));

        assertThat(on).isTrue();
        verify(ops, times(1)).set(anyString(), anyString(), any());
    }

    @Test
    void unknownFlag_returnsFalse() throws Exception {
        when(ops.get(anyString())).thenReturn("{}");
        assertThat(service.isOn("nope", Map.of())).isFalse();
    }

    @Test
    void activeFlagsHeader_joinsAllTrueFlags() throws Exception {
        String cached = "{\"a\":{\"defaultValue\":true},\"b\":{\"defaultValue\":false},\"c\":{\"defaultValue\":true}}";
        when(ops.get(anyString())).thenReturn(cached);

        String h = service.activeFlagsHeader(Map.of("user_id", "u1"));

        assertThat(h.split(",")).containsExactlyInAnyOrder("a", "c");
    }

    @Test
    void attributesHash_isStable() {
        String h1 = FeatureFlagsService.attributesHash(Map.of("user_id", "abc"));
        String h2 = FeatureFlagsService.attributesHash(Map.of("user_id", "abc"));
        String h3 = FeatureFlagsService.attributesHash(Map.of("user_id", "xyz"));
        assertThat(h1).isEqualTo(h2).hasSize(16);
        assertThat(h1).isNotEqualTo(h3);
    }

    @Test
    void evaluate_forceRule_takesPrecedenceOverDefault() throws Exception {
        String json = """
            {"poulets.new-checkout":{
              "defaultValue":false,
              "rules":[{"force":true,"condition":{"id":"eleveur-42"}}]
            }}""";
        JsonNode features = mapper.readTree(json);
        assertThat(service.evaluate(features, "poulets.new-checkout", Map.of("user_id", "eleveur-42"))).isTrue();
        assertThat(service.evaluate(features, "poulets.new-checkout", Map.of("user_id", "autre"))).isFalse();
    }
}
