/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.service.admin;

import bf.gov.faso.notifier.metrics.NotifierMetrics;
import jakarta.mail.MessagingException;
import jakarta.mail.internet.MimeMessage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.core.io.ByteArrayResource;
import org.springframework.mail.MailException;
import org.springframework.mail.javamail.JavaMailSender;
import org.springframework.mail.javamail.MimeMessageHelper;
import org.springframework.stereotype.Service;

import java.io.UnsupportedEncodingException;
import java.util.Map;

/**
 * AdminMailDispatcher — sends rendered admin emails via {@link JavaMailSender}.
 *
 * <p>Configuration is driven by {@code spring.mail.*} properties (Mailpit dev,
 * Mailersend prod). Optional plain-text attachment supported (e.g. recovery codes).
 *
 * <p>This component does NOT perform retry — callers (consumers) handle retry
 * semantics via Resilience4j on the calling site, or by re-throwing for Kafka DLQ.
 */
@Service
public class AdminMailDispatcher {

    private static final Logger log = LoggerFactory.getLogger(AdminMailDispatcher.class);

    private final JavaMailSender mailSender;
    private final NotifierMetrics metrics;

    @Value("${notifier.mail.from:noreply@faso.gov.bf}")
    private String mailFrom;

    @Value("${notifier.mail.from-name:FASO DIGITALISATION}")
    private String mailFromName;

    public AdminMailDispatcher(JavaMailSender mailSender, NotifierMetrics metrics) {
        this.mailSender = mailSender;
        this.metrics = metrics;
    }

    /** Send a multipart HTML+text email without attachments. */
    public void send(String to, AdminMailRenderer.RenderedAdminMail mail) {
        send(to, mail, Map.of());
    }

    /**
     * Send a multipart HTML+text email with optional plain-text attachments.
     *
     * @param to          recipient email address
     * @param mail        rendered subject + bodies
     * @param attachments map of filename → bytes (UTF-8 plain-text)
     */
    public void send(String to, AdminMailRenderer.RenderedAdminMail mail, Map<String, byte[]> attachments) {
        try {
            MimeMessage message = mailSender.createMimeMessage();
            MimeMessageHelper helper = new MimeMessageHelper(message, true, "UTF-8");
            helper.setFrom(mailFrom, mailFromName);
            helper.setTo(to);
            helper.setSubject(mail.subject());
            helper.setText(mail.text(), mail.html());
            if (attachments != null) {
                for (Map.Entry<String, byte[]> e : attachments.entrySet()) {
                    helper.addAttachment(e.getKey(),
                        new ByteArrayResource(e.getValue()), "text/plain; charset=UTF-8");
                }
            }
            mailSender.send(message);
            metrics.incrementMailSent();
            log.info("Admin mail sent: to={} subject='{}' attachments={}",
                to, mail.subject(), attachments == null ? 0 : attachments.size());
        } catch (MessagingException | UnsupportedEncodingException | MailException e) {
            metrics.incrementMailFailed();
            throw new AdminMailDispatchException(
                "SMTP send failure for " + to + ": " + e.getMessage(), e);
        }
    }

    /** Wrapper exception so callers can distinguish dispatch failure from rendering. */
    public static class AdminMailDispatchException extends RuntimeException {
        public AdminMailDispatchException(String msg, Throwable cause) { super(msg, cause); }
    }
}
