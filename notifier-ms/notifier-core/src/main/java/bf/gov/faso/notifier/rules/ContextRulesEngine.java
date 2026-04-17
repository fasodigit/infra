/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.rules;

import bf.gov.faso.notifier.domain.GithubEventPayload;
import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.core.io.Resource;
import org.springframework.stereotype.Component;

import jakarta.annotation.PostConstruct;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * ContextRulesEngine — evaluates loaded rules against an incoming GitHub event
 * and returns the list of matching {@link ContextRule} entries.
 *
 * <p>Rules are loaded once at startup from {@code context-rules.json}.
 * Dynamic overrides can be stored in the {@code notification_templates} table
 * and are resolved by {@link bf.gov.faso.notifier.service.TemplateResolverService}.
 */
@Component
public class ContextRulesEngine {

    private static final Logger log = LoggerFactory.getLogger(ContextRulesEngine.class);

    private final ObjectMapper objectMapper;
    private final Resource contextRulesResource;
    private List<ContextRule> rules = List.of();

    public ContextRulesEngine(
            ObjectMapper objectMapper,
            @Value("${notifier.context-rules.file:classpath:context-rules.json}") Resource contextRulesResource) {
        this.objectMapper = objectMapper;
        this.contextRulesResource = contextRulesResource;
    }

    @PostConstruct
    public void loadRules() throws IOException {
        rules = objectMapper.readValue(
            contextRulesResource.getInputStream(),
            new TypeReference<List<ContextRule>>() {}
        );
        log.info("Loaded {} context rules from {}", rules.size(), contextRulesResource.getDescription());
    }

    /**
     * Evaluate all rules against the provided event.
     *
     * @param payload the deserialized GitHub event
     * @return ordered list of matching rules (first match wins in dispatch logic)
     */
    public List<ContextRule> evaluate(GithubEventPayload payload) {
        List<ContextRule> matched = new ArrayList<>();
        for (ContextRule rule : rules) {
            if (matches(rule, payload)) {
                matched.add(rule);
                log.debug("Rule '{}' matched for repo='{}' event='{}'",
                    rule.id(),
                    payload.repository() != null ? payload.repository().fullName() : "unknown",
                    payload.eventType());
            }
        }
        return matched;
    }

    /**
     * Reload rules at runtime (e.g. after API-level update).
     */
    public void reload(List<ContextRule> newRules) {
        this.rules = List.copyOf(newRules);
        log.info("Context rules reloaded: {} rules active", rules.size());
    }

    public List<ContextRule> getRules() {
        return List.copyOf(rules);
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    private boolean matches(ContextRule rule, GithubEventPayload payload) {
        if (rule.conditions() == null || rule.conditions().isEmpty()) {
            return false;
        }
        // Build a flat attribute map from the payload for condition evaluation
        Map<String, String> attrs = buildAttributes(payload);
        return rule.conditions().entrySet().stream()
            .allMatch(entry -> {
                String actual = attrs.get(entry.getKey());
                if (actual == null) return false;
                // Support glob-style wildcard patterns (e.g. "fasodigit/*")
                String pattern = entry.getValue();
                if (pattern.endsWith("*")) {
                    return actual.startsWith(pattern.substring(0, pattern.length() - 1));
                }
                return actual.equalsIgnoreCase(pattern);
            });
    }

    private Map<String, String> buildAttributes(GithubEventPayload payload) {
        var map = new java.util.HashMap<String, String>();
        if (payload.eventType() != null)       map.put("event_type", payload.eventType());
        if (payload.ref() != null)             map.put("ref", payload.ref());
        if (payload.repository() != null) {
            var repo = payload.repository();
            if (repo.fullName() != null)       map.put("repository_full_name", repo.fullName());
            if (repo.name() != null)           map.put("repository_name", repo.name());
        }
        if (payload.sender() != null && payload.sender().login() != null) {
            map.put("sender_login", payload.sender().login());
        }
        if (payload.pullRequest() != null) {
            var pr = payload.pullRequest();
            if (pr.state() != null)            map.put("pr_state", pr.state());
            if (pr.merged() != null)           map.put("pr_merged", pr.merged().toString());
        }
        return map;
    }
}
