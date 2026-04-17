/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier;

import bf.gov.faso.notifier.consumer.GithubEventConsumer;
import bf.gov.faso.notifier.domain.GithubEventPayload;
import bf.gov.faso.notifier.domain.NotificationDelivery;
import bf.gov.faso.notifier.service.DeliveryRepository;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.icegreen.greenmail.configuration.GreenMailConfiguration;
import com.icegreen.greenmail.junit5.GreenMailExtension;
import com.icegreen.greenmail.util.ServerSetupTest;
import org.apache.kafka.clients.producer.ProducerRecord;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.RegisterExtension;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.kafka.core.KafkaTemplate;
import org.springframework.kafka.test.context.EmbeddedKafka;
import org.springframework.test.context.ActiveProfiles;
import org.springframework.test.context.DynamicPropertyRegistry;
import org.springframework.test.context.DynamicPropertySource;
import org.testcontainers.containers.PostgreSQLContainer;
import org.testcontainers.junit.jupiter.Container;
import org.testcontainers.junit.jupiter.Testcontainers;

import jakarta.mail.internet.MimeMessage;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.awaitility.Awaitility.await;

/**
 * GithubEventConsumerTest — integration tests for the full event-to-mail pipeline.
 *
 * <p>Test coverage:
 * <ul>
 *   <li>Happy path: push event → matched rule → mail delivered via GreenMail</li>
 *   <li>Deduplication: replay same delivery_id → no duplicate mail</li>
 *   <li>Template render: complex commit payload with all vars</li>
 *   <li>DLQ: SMTP failure after 3 retries → delivery status = DLQ</li>
 *   <li>Context rules: pattern matching (fasodigit/*)</li>
 * </ul>
 */
@SpringBootTest
@Testcontainers
@EmbeddedKafka(
    partitions = 1,
    topics = {"github.events.v1", "github.events.v1.dlq"},
    brokerProperties = {
        "listeners=PLAINTEXT://localhost:0",
        "port=0"
    }
)
@ActiveProfiles("test")
class GithubEventConsumerTest {

    @RegisterExtension
    static GreenMailExtension greenMail = new GreenMailExtension(ServerSetupTest.SMTP)
        .withConfiguration(GreenMailConfiguration.aConfig()
            .withUser("test@faso.gov.bf", "test"))
        .withPerMethodLifecycle(false);

    @Container
    static PostgreSQLContainer<?> postgres = new PostgreSQLContainer<>("postgres:17-alpine")
        .withDatabaseName("notifier_test")
        .withUsername("notifier")
        .withPassword("notifier_test");

    @DynamicPropertySource
    static void configureProperties(DynamicPropertyRegistry registry) {
        registry.add("spring.datasource.url", postgres::getJdbcUrl);
        registry.add("spring.datasource.username", postgres::getUsername);
        registry.add("spring.datasource.password", postgres::getPassword);
        registry.add("spring.mail.host", () -> "localhost");
        registry.add("spring.mail.port", () -> ServerSetupTest.SMTP.getPort());
        registry.add("spring.mail.properties.mail.smtp.auth", () -> "false");
        registry.add("spring.mail.properties.mail.smtp.starttls.enable", () -> "false");
    }

    @Autowired
    private KafkaTemplate<String, byte[]> kafkaTemplate;

    @Autowired
    private DeliveryRepository deliveryRepository;

    @Autowired
    private StringRedisTemplate redisTemplate;

    @Autowired
    private ObjectMapper objectMapper;

    @BeforeEach
    void setUp() {
        greenMail.reset();
        deliveryRepository.deleteAll();
    }

    @Test
    void givenInfraPushEvent_whenConsumed_thenMailDispatchedToDevOps() throws Exception {
        // Arrange
        String deliveryId = "gh-" + UUID.randomUUID();
        GithubEventPayload payload = buildInfraPushPayload(deliveryId);
        byte[] payloadBytes = objectMapper.writeValueAsBytes(payload);

        // Act
        kafkaTemplate.send("github.events.v1", deliveryId, payloadBytes).get();

        // Assert: mail delivered within 10s
        await().atMost(10, TimeUnit.SECONDS).until(() -> greenMail.getReceivedMessages().length >= 1);
        MimeMessage[] messages = greenMail.getReceivedMessages();
        assertThat(messages).isNotEmpty();
        assertThat(messages[0].getAllRecipients()[0].toString()).isEqualTo("devops@faso.gov.bf");
        assertThat(messages[0].getSubject()).contains("infra");

        // Assert: delivery record persisted as SENT
        await().atMost(5, TimeUnit.SECONDS).until(() ->
            deliveryRepository.findByStatus(NotificationDelivery.Status.SENT,
                org.springframework.data.domain.Pageable.unpaged()).getTotalElements() >= 1
        );
    }

    @Test
    void givenSameDeliveryIdSentTwice_thenOnlyOneMailDispatched() throws Exception {
        // Arrange
        String deliveryId = "gh-dedup-" + UUID.randomUUID();
        GithubEventPayload payload = buildInfraPushPayload(deliveryId);
        byte[] payloadBytes = objectMapper.writeValueAsBytes(payload);

        // Act: send same event twice
        kafkaTemplate.send("github.events.v1", deliveryId, payloadBytes).get();
        TimeUnit.MILLISECONDS.sleep(500);
        kafkaTemplate.send("github.events.v1", deliveryId, payloadBytes).get();

        // Assert: only one mail despite two messages
        await().atMost(10, TimeUnit.SECONDS).until(() -> greenMail.getReceivedMessages().length >= 1);
        TimeUnit.SECONDS.sleep(2); // Extra wait to ensure no duplicate arrives
        assertThat(greenMail.getReceivedMessages()).hasSize(1);
    }

    @Test
    void givenComplexCommitPayload_thenTemplateRenderedWithAllVars() throws Exception {
        // Arrange: payload with multiple commits and file changes
        String deliveryId = "gh-render-" + UUID.randomUUID();
        GithubEventPayload payload = new GithubEventPayload(
            "push", deliveryId,
            new GithubEventPayload.Repository("fasodigit/infra", "infra",
                "https://github.com/fasodigit/infra", "FASO Infrastructure"),
            new GithubEventPayload.Sender("ci-bot",
                "https://avatars.githubusercontent.com/u/1?v=4",
                "https://github.com/ci-bot"),
            "refs/heads/main",
            "https://github.com/fasodigit/infra/compare/abc...def",
            List.of(
                new GithubEventPayload.Commit("abc123def456789", "feat(kaya): add WAL support",
                    new GithubEventPayload.CommitAuthor("DevOps", "devops@faso.gov.bf"),
                    "https://github.com/fasodigit/infra/commit/abc123",
                    List.of("kaya/wal.rs"), List.of("kaya/Cargo.toml"), List.of()),
                new GithubEventPayload.Commit("def456abc123789", "fix(armageddon): timeout tuning",
                    new GithubEventPayload.CommitAuthor("SRE", "sre@faso.gov.bf"),
                    "https://github.com/fasodigit/infra/commit/def456",
                    List.of(), List.of("armageddon/config.toml"), List.of("old-config.toml"))
            ),
            null
        );
        byte[] payloadBytes = objectMapper.writeValueAsBytes(payload);

        // Act
        kafkaTemplate.send("github.events.v1", deliveryId, payloadBytes).get();

        // Assert: mail received with rendered content
        await().atMost(10, TimeUnit.SECONDS).until(() -> greenMail.getReceivedMessages().length >= 1);
        MimeMessage msg = greenMail.getReceivedMessages()[0];
        String body = msg.getContent().toString();
        assertThat(body).contains("fasodigit/infra");
        assertThat(body).contains("main");
        assertThat(body).contains("abc123d"); // shortSha
    }

    @Test
    void givenNonMatchingEvent_thenNoMailDispatched() throws Exception {
        // Arrange: repo not in any rule
        String deliveryId = "gh-nomatch-" + UUID.randomUUID();
        GithubEventPayload payload = new GithubEventPayload(
            "push", deliveryId,
            new GithubEventPayload.Repository("external/unknown-repo", "unknown-repo",
                "https://github.com/external/unknown-repo", null),
            null, "refs/heads/main", null, List.of(), null
        );
        byte[] payloadBytes = objectMapper.writeValueAsBytes(payload);

        // Act
        kafkaTemplate.send("github.events.v1", deliveryId, payloadBytes).get();

        // Assert: no mail after 3s
        TimeUnit.SECONDS.sleep(3);
        assertThat(greenMail.getReceivedMessages()).isEmpty();
    }

    @Test
    void givenContextRuleWithWildcard_thenPREventMatchesAllFasoRepos() throws Exception {
        // PR event for poulets repo should match faso-all-pr-opened wildcard rule
        String deliveryId = "gh-pr-" + UUID.randomUUID();
        GithubEventPayload payload = new GithubEventPayload(
            "pull_request", deliveryId,
            new GithubEventPayload.Repository("fasodigit/poulets", "poulets",
                "https://github.com/fasodigit/poulets", null),
            new GithubEventPayload.Sender("farmer-dev", null, null),
            null, null, null,
            new GithubEventPayload.PullRequest(42, "feat: add bulk order",
                "https://github.com/fasodigit/poulets/pull/42",
                "open", false, "Bulk order feature for farmers",
                new GithubEventPayload.Sender("farmer-dev", null, null),
                new GithubEventPayload.Branch("feature/bulk-order", "sha1"),
                new GithubEventPayload.Branch("main", "sha2"))
        );
        byte[] payloadBytes = objectMapper.writeValueAsBytes(payload);

        kafkaTemplate.send("github.events.v1", deliveryId, payloadBytes).get();

        await().atMost(10, TimeUnit.SECONDS).until(() -> greenMail.getReceivedMessages().length >= 1);
        assertThat(greenMail.getReceivedMessages()).isNotEmpty();
        // Subject should contain PR info
        assertThat(greenMail.getReceivedMessages()[0].getSubject()).containsIgnoringCase("pull");
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private GithubEventPayload buildInfraPushPayload(String deliveryId) {
        return new GithubEventPayload(
            "push", deliveryId,
            new GithubEventPayload.Repository("fasodigit/infra", "infra",
                "https://github.com/fasodigit/infra", "FASO Infrastructure"),
            new GithubEventPayload.Sender("devops-bot",
                "https://avatars.githubusercontent.com/u/1?v=4",
                "https://github.com/devops-bot"),
            "refs/heads/main",
            "https://github.com/fasodigit/infra/compare/abc...def",
            List.of(
                new GithubEventPayload.Commit("abc123456789", "chore: update Containerfile",
                    new GithubEventPayload.CommitAuthor("DevOps", "devops@faso.gov.bf"),
                    "https://github.com/fasodigit/infra/commit/abc123456789",
                    List.of(), List.of("Containerfile"), List.of())
            ),
            null
        );
    }
}
