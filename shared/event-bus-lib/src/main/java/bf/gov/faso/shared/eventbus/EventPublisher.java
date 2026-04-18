// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.shared.eventbus;

import java.util.Map;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.kafka.core.KafkaTemplate;
import org.springframework.stereotype.Component;
import com.fasterxml.jackson.databind.ObjectMapper;

/**
 * Publisher Kafka/Redpanda pour événements FASO.
 * Stub minimal — à enrichir avec schema registry, DLQ, etc.
 */
@Component
public class EventPublisher {
  private final KafkaTemplate<String, String> kafkaTemplate;
  private final ObjectMapper mapper = new ObjectMapper();

  @Autowired(required = false)
  public EventPublisher(KafkaTemplate<String, String> kafkaTemplate) {
    this.kafkaTemplate = kafkaTemplate;
  }

  /** Publie un événement JSON sur un topic. */
  public void publish(String topic, String key, Map<String, Object> payload) {
    if (kafkaTemplate == null) return;
    try {
      kafkaTemplate.send(topic, key, mapper.writeValueAsString(payload));
    } catch (Exception e) {
      throw new RuntimeException("Event publish failed: " + topic, e);
    }
  }
}
