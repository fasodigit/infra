/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.service;

import bf.gov.faso.audit.Audited;
import bf.gov.faso.notifier.domain.GithubEventPayload;
import bf.gov.faso.notifier.domain.NotificationDelivery;
import bf.gov.faso.notifier.domain.NotificationDelivery.Status;
import bf.gov.faso.notifier.metrics.NotifierMetrics;
import bf.gov.faso.notifier.rules.ContextRule;
import io.github.resilience4j.retry.annotation.Retry;
import jakarta.mail.internet.MimeMessage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.kafka.core.KafkaTemplate;
import org.springframework.mail.MailException;
import org.springframework.mail.javamail.JavaMailSender;
import org.springframework.mail.javamail.MimeMessageHelper;
import org.springframework.scheduling.annotation.Async;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.List;

/**
 * NotificationService — orchestrates template rendering, mail dispatch, retry logic,
 * and DLQ forwarding for each matched context rule.
 *
 * <p>Retry policy (Resilience4j): 3 attempts with exponential backoff (2s × 2^n).
 * After exhaustion, the event is forwarded to the DLQ topic and the delivery record
 * is updated to {@code Status.DLQ}.
 */
@Service
public class NotificationService {

    private static final Logger log = LoggerFactory.getLogger(NotificationService.class);

    private final JavaMailSender mailSender;
    private final TemplateRenderService renderService;
    private final DeliveryRepository deliveryRepository;
    private final KafkaTemplate<String, byte[]> kafkaTemplate;
    private final NotifierMetrics metrics;

    @Value("${notifier.mail.from:noreply@faso.gov.bf}")
    private String mailFrom;

    @Value("${notifier.mail.from-name:FASO DIGITALISATION}")
    private String mailFromName;

    @Value("${notifier.topics.dlq:github.events.v1.dlq}")
    private String dlqTopic;

    public NotificationService(
            JavaMailSender mailSender,
            TemplateRenderService renderService,
            DeliveryRepository deliveryRepository,
            KafkaTemplate<String, byte[]> kafkaTemplate,
            NotifierMetrics metrics) {
        this.mailSender = mailSender;
        this.renderService = renderService;
        this.deliveryRepository = deliveryRepository;
        this.kafkaTemplate = kafkaTemplate;
        this.metrics = metrics;
    }

    /**
     * Dispatch notifications for all matching rules, asynchronously.
     *
     * @param payload     the parsed GitHub event
     * @param matchedRules rules matched by the ContextRulesEngine
     * @param rawPayload  original event bytes for DLQ forwarding
     */
    @Async
    public void dispatchAll(GithubEventPayload payload, List<ContextRule> matchedRules, byte[] rawPayload) {
        for (ContextRule rule : matchedRules) {
            for (String recipient : rule.recipients()) {
                String deliveryId = buildDeliveryId(payload.deliveryId(), rule.id(), recipient);
                dispatch(deliveryId, recipient, rule.template(), payload, rawPayload);
            }
        }
    }

    // ── Internal dispatch with retry ──────────────────────────────────────────

    @Transactional
    @Audited(action = "SEND_NOTIFICATION", resourceType = "NotificationDelivery")
    public void dispatch(
            String deliveryId,
            String recipient,
            String templateName,
            GithubEventPayload payload,
            byte[] rawPayload) {

        NotificationDelivery delivery = findOrCreate(deliveryId, recipient, templateName, rawPayload);
        if (delivery.getStatus() == Status.SENT) {
            log.debug("Delivery {} already SENT, skipping", deliveryId);
            return;
        }

        try {
            sendWithRetry(delivery, templateName, recipient, payload);
            delivery.setStatus(Status.SENT);
            delivery.setSentAt(Instant.now());
            delivery.setLastError(null);
            deliveryRepository.save(delivery);
            metrics.incrementMailSent();
            log.info("Mail sent: deliveryId={} recipient={} template={}", deliveryId, recipient, templateName);

        } catch (Exception e) {
            log.error("Mail dispatch failed after retries: deliveryId={} error={}", deliveryId, e.getMessage());
            delivery.setStatus(Status.DLQ);
            delivery.setLastError(e.getMessage());
            deliveryRepository.save(delivery);
            metrics.incrementMailFailed();
            metrics.incrementDlq();
            forwardToDlq(deliveryId, rawPayload);
        }
    }

    @Retry(name = "mail-dispatch")
    void sendWithRetry(
            NotificationDelivery delivery,
            String templateName,
            String recipient,
            GithubEventPayload payload) {

        delivery.setAttempts(delivery.getAttempts() + 1);
        deliveryRepository.save(delivery);

        TemplateRenderService.RenderedEmail rendered =
            renderService.renderFromClasspath(templateName, payload);

        try {
            MimeMessage message = mailSender.createMimeMessage();
            MimeMessageHelper helper = new MimeMessageHelper(message, true, "UTF-8");
            helper.setFrom(mailFrom, mailFromName);
            helper.setTo(recipient);
            helper.setSubject(rendered.subject());
            helper.setText(rendered.body(), true); // HTML=true
            mailSender.send(message);
        } catch (Exception e) {
            throw new MailException("SMTP send failure for " + recipient + ": " + e.getMessage()) {};
        }
    }

    // ── DLQ ──────────────────────────────────────────────────────────────────

    private void forwardToDlq(String deliveryId, byte[] rawPayload) {
        try {
            kafkaTemplate.send(dlqTopic, deliveryId, rawPayload);
            log.warn("Forwarded to DLQ: deliveryId={} topic={}", deliveryId, dlqTopic);
        } catch (Exception e) {
            log.error("Failed to forward to DLQ: deliveryId={} error={}", deliveryId, e.getMessage());
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private NotificationDelivery findOrCreate(
            String deliveryId, String recipient, String templateName, byte[] rawPayload) {
        return deliveryRepository.findById(deliveryId).orElseGet(() -> {
            NotificationDelivery d = new NotificationDelivery();
            d.setDeliveryId(deliveryId);
            d.setRecipient(recipient);
            d.setTemplateName(templateName);
            d.setStatus(Status.PENDING);
            d.setEventPayload(new String(rawPayload));
            return deliveryRepository.save(d);
        });
    }

    private String buildDeliveryId(String eventDeliveryId, String ruleId, String recipient) {
        // Stable deterministic ID: eventId + ruleId + recipient hash
        int recipientHash = recipient.hashCode() & 0xffff;
        return String.format("%s_%s_%04x", eventDeliveryId, ruleId, recipientHash);
    }
}
