// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
package bf.gov.faso.poulets.flags;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.EnabledIfEnvironmentVariable;
import org.springframework.data.redis.connection.jedis.JedisConnectionFactory;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.web.client.RestTemplate;
import org.testcontainers.containers.GenericContainer;
import org.testcontainers.containers.MongoDBContainer;
import org.testcontainers.containers.wait.strategy.Wait;
import org.testcontainers.junit.jupiter.Container;
import org.testcontainers.junit.jupiter.Testcontainers;
import org.testcontainers.utility.DockerImageName;

import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Test d'intégration Testcontainers : fait tourner
 *   - MongoDB (persistance GrowthBook, smoke test uniquement)
 *   - KAYA (cache, exposé en RESP3 sur 6380)
 *
 * <p>Skippé par défaut ; activé en CI via {@code TESTCONTAINERS_ENABLED=true}
 * car le pull image prend du temps. Pour local :
 * {@code TESTCONTAINERS_ENABLED=true mvn -Dtest=FeatureFlagsIntegrationIT verify}.</p>
 */
@Tag("integration")
@Testcontainers
@EnabledIfEnvironmentVariable(named = "TESTCONTAINERS_ENABLED", matches = "true")
class FeatureFlagsIntegrationIT {

    @Container
    static final MongoDBContainer MONGO =
            new MongoDBContainer(DockerImageName.parse("mongo:7"))
                    .withReuse(true);

    // KAYA expose une API RESP3 compatible Redis sur 6380 — image sovereign FASO.
    // Pour le test, on utilise l'image officielle publiée dans le registre interne.
    @Container
    static final GenericContainer<?> KAYA =
            new GenericContainer<>(DockerImageName.parse("ghcr.io/faso-digitalisation/kaya:latest"))
                    .withExposedPorts(6380)
                    .waitingFor(Wait.forListeningPort());

    @Test
    void cacheSetGet_againstRealKaya() {
        String host = KAYA.getHost();
        int port = KAYA.getMappedPort(6380);

        JedisConnectionFactory f = new JedisConnectionFactory();
        f.getStandaloneConfiguration().setHostName(host);
        f.getStandaloneConfiguration().setPort(port);
        f.afterPropertiesSet();

        StringRedisTemplate redis = new StringRedisTemplate(f);
        redis.afterPropertiesSet();

        FeatureFlagsService svc = new FeatureFlagsService(redis, new RestTemplate(), new ObjectMapper());
        svc.configure("http://unused", "dev", "test");

        // Seed cache directement (simule une population antérieure par le backend).
        String key = "ff:dev:" + FeatureFlagsService.attributesHash(Map.of("user_id", "u1"));
        redis.opsForValue().set(key, "{\"poulets.new-checkout\":{\"defaultValue\":true}}");

        assertThat(svc.isOn("poulets.new-checkout", Map.of("user_id", "u1"))).isTrue();
        assertThat(svc.peekCache(Map.of("user_id", "u1"))).isPresent();
    }

    @Test
    void mongoSmokeTest() {
        // Vérifie uniquement que Mongo est up — GrowthBook lui-même n'est pas démarré ici.
        assertThat(MONGO.isRunning()).isTrue();
        assertThat(MONGO.getReplicaSetUrl()).startsWith("mongodb://");
    }
}
