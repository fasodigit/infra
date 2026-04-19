/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier;

import bf.gov.faso.notifier.domain.GithubEventPayload;
import bf.gov.faso.notifier.domain.NotificationDelivery;
import bf.gov.faso.notifier.service.DeliveryRepository;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.kafka.test.context.EmbeddedKafka;
import org.springframework.mail.MailSendException;
import org.springframework.mail.javamail.JavaMailSender;
import org.springframework.test.context.ActiveProfiles;

import java.util.List;
import java.util.UUID;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;
import static org.awaitility.Awaitility.await;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

/**
 * DlqTest — verifies that SMTP failures after 3 retries result in DLQ forwarding.
 *
 * <p>TODO(FASO-NOTIFIER-TESTS): integration test disabled — requires a full stack
 * harness (Testcontainers Postgres + KAYA/Redis stub + embedded Kafka with
 * {@code spring.kafka.listener.auto-startup=true} override in the test profile)
 * to bring up the real {@link bf.gov.faso.notifier.consumer.GithubEventConsumer}
 * pipeline. The current test profile inherits {@code auto-startup=false} from
 * {@code application.yml} (added to suppress broker spam when Redpanda is down)
 * and has no datasource override, so the Spring context fails to start with
 * {@code UnknownHostException: postgres}. Re-enable once a proper IT profile is
 * introduced (track in follow-up issue).
 */
@Disabled("Pending IT harness: needs Testcontainers Postgres + KAYA stub + Kafka auto-startup override")
@SpringBootTest
@EmbeddedKafka(
    partitions = 1,
    topics = {"github.events.v1", "github.events.v1.dlq"}
)
@ActiveProfiles("test")
class DlqTest {

    @MockBean
    private JavaMailSender mailSender;

    @Autowired
    private DeliveryRepository deliveryRepository;

    @Autowired
    private ObjectMapper objectMapper;

    @Autowired
    private org.springframework.kafka.core.KafkaTemplate<String, byte[]> kafkaTemplate;

    @Test
    void givenSmtpAlwaysFails_thenDeliveryStatusIsDlqAfterRetries() throws Exception {
        // Arrange: SMTP always throws
        when(mailSender.createMimeMessage())
            .thenThrow(new MailSendException("SMTP connection refused - test"));

        String deliveryId = "gh-dlq-" + UUID.randomUUID();
        GithubEventPayload payload = new GithubEventPayload(
            "push", deliveryId,
            new GithubEventPayload.Repository("fasodigit/infra", "infra",
                "https://github.com/fasodigit/infra", null),
            null, "refs/heads/main", null,
            List.of(new GithubEventPayload.Commit("abc123", "fix: smtp test",
                new GithubEventPayload.CommitAuthor("Test", "test@faso.gov.bf"),
                "https://github.com/fasodigit/infra/commit/abc123",
                List.of(), List.of(), List.of())),
            null
        );
        byte[] payloadBytes = objectMapper.writeValueAsBytes(payload);

        // Act
        kafkaTemplate.send("github.events.v1", deliveryId, payloadBytes).get();

        // Assert: after retry exhaustion, delivery is in DLQ status
        await().atMost(30, TimeUnit.SECONDS).until(() ->
            deliveryRepository.findAll().stream()
                .anyMatch(d -> d.getStatus() == NotificationDelivery.Status.DLQ)
        );

        List<NotificationDelivery> deliveries = deliveryRepository.findAll();
        assertThat(deliveries).anyMatch(d ->
            d.getStatus() == NotificationDelivery.Status.DLQ &&
            d.getAttempts() >= 3
        );

        // Verify mailSender was called (retry attempted)
        verify(mailSender, atLeast(1)).createMimeMessage();
    }
}
