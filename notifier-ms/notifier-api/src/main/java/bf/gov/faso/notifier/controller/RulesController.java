/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.controller;

import bf.gov.faso.notifier.rules.ContextRule;
import bf.gov.faso.notifier.rules.ContextRulesEngine;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.List;

/**
 * RulesController — CRUD for in-memory context rules.
 *
 * <p>Rules are loaded from {@code context-rules.json} at startup and can be
 * hot-reloaded at runtime via this endpoint.
 * Endpoint: {@code /api/rules}
 */
@RestController
@RequestMapping("/api/rules")
public class RulesController {

    private final ContextRulesEngine rulesEngine;

    public RulesController(ContextRulesEngine rulesEngine) {
        this.rulesEngine = rulesEngine;
    }

    @GetMapping
    @PreAuthorize("hasAuthority('SCOPE_notifier:read')")
    public List<ContextRule> list() {
        return rulesEngine.getRules();
    }

    @PutMapping
    @PreAuthorize("hasAuthority('SCOPE_notifier:admin')")
    @ResponseStatus(HttpStatus.OK)
    public List<ContextRule> replace(@RequestBody List<ContextRule> rules) {
        rulesEngine.reload(rules);
        return rulesEngine.getRules();
    }

    @PostMapping("/evaluate")
    @PreAuthorize("hasAuthority('SCOPE_notifier:read')")
    public List<ContextRule> evaluate(
            @RequestBody bf.gov.faso.notifier.domain.GithubEventPayload payload) {
        return rulesEngine.evaluate(payload);
    }
}
