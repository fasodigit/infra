/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.rules;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.List;
import java.util.Map;

/**
 * ContextRule — immutable routing rule loaded from {@code context-rules.json}.
 *
 * <p>Example:
 * <pre>
 * {
 *   "id": "infra-push",
 *   "if": { "repository_full_name": "fasodigit/infra", "event_type": "push" },
 *   "recipients": ["devops@faso.gov.bf"],
 *   "template": "infra-commit"
 * }
 * </pre>
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public record ContextRule(

    @JsonProperty("id")          String id,
    @JsonProperty("if")          Map<String, String> conditions,
    @JsonProperty("recipients")  List<String> recipients,
    @JsonProperty("template")    String template
) {}
