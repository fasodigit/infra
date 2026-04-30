/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.client;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Component;
import org.springframework.web.client.RestClient;

import java.util.Map;

/**
 * SlackWebhookClient — best-effort Slack notification stub for high-priority
 * security alerts (BREAK-GLASS). Webhook URL is read from the environment
 * (typically injected by Vault). When unset, {@link #postSecurityAlert(String)}
 * is a no-op and only logs at INFO level.
 *
 * <p>TODO: replace this stub with Vault-backed credential lookup once the
 * {@code faso/notifier/slack} secret path is provisioned.
 */
@Component
public class SlackWebhookClient {

    private static final Logger log = LoggerFactory.getLogger(SlackWebhookClient.class);

    private final String webhookUrl;
    private final RestClient restClient;

    public SlackWebhookClient(@Value("${notifier.slack.webhook-url:}") String webhookUrl) {
        this.webhookUrl = webhookUrl;
        this.restClient = RestClient.create();
    }

    /** Post a plain-text alert to the configured Slack channel. */
    public void postSecurityAlert(String text) {
        if (webhookUrl == null || webhookUrl.isBlank()) {
            // TODO: read from Vault path faso/notifier/slack/webhook-url
            log.info("Slack webhook URL not configured — skipping post (text='{}')", text);
            return;
        }
        try {
            restClient.post()
                .uri(webhookUrl)
                .contentType(MediaType.APPLICATION_JSON)
                .body(Map.of("text", text))
                .retrieve()
                .toBodilessEntity();
            log.debug("Slack alert posted: {}", text);
        } catch (Exception e) {
            log.warn("Slack webhook POST failed: {}", e.getMessage());
        }
    }
}
