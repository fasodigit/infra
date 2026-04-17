/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier;

import bf.gov.faso.notifier.domain.GithubEventPayload;
import bf.gov.faso.notifier.rules.ContextRule;
import bf.gov.faso.notifier.rules.ContextRulesEngine;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.core.io.ClassPathResource;

import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * ContextRulesEngineTest — unit tests for context rules evaluation and pattern matching.
 */
class ContextRulesEngineTest {

    private ContextRulesEngine engine;

    @BeforeEach
    void setUp() throws Exception {
        engine = new ContextRulesEngine(
            new ObjectMapper(),
            new ClassPathResource("context-rules.json")
        );
        engine.loadRules();
    }

    @Test
    void infraPushEvent_matchesInfraCommitRule() {
        GithubEventPayload payload = pushEvent("fasodigit/infra");
        List<ContextRule> matched = engine.evaluate(payload);

        assertThat(matched).anyMatch(r -> r.template().equals("infra-commit"));
        assertThat(matched).anyMatch(r -> r.recipients().contains("devops@faso.gov.bf"));
    }

    @Test
    void vouchersPushEvent_matchesVouchersTemplate() {
        GithubEventPayload payload = pushEvent("fasodigit/vouchers");
        List<ContextRule> matched = engine.evaluate(payload);

        assertThat(matched).anyMatch(r -> r.template().equals("vouchers-commit"));
    }

    @Test
    void unknownRepoPushEvent_noRulesMatch() {
        GithubEventPayload payload = pushEvent("external/unknown");
        List<ContextRule> matched = engine.evaluate(payload);

        assertThat(matched).isEmpty();
    }

    @Test
    void prEventForAnyFasoRepo_matchesWildcardRule() {
        // PR to any fasodigit/* repo should match the wildcard PR rules
        GithubEventPayload payload = prEvent("fasodigit/escool", "open", false);
        List<ContextRule> matched = engine.evaluate(payload);

        assertThat(matched).anyMatch(r -> r.template().equals("pull-request-opened"));
    }

    @Test
    void mergedPREvent_matchesMergedTemplate() {
        GithubEventPayload payload = prEvent("fasodigit/infra", "closed", true);
        List<ContextRule> matched = engine.evaluate(payload);

        assertThat(matched).anyMatch(r -> r.template().equals("pull-request-merged"));
    }

    @Test
    void reloadRules_newRulesApplied() {
        ContextRule customRule = new ContextRule(
            "custom-test",
            Map.of("repository_full_name", "test/repo", "event_type", "push"),
            List.of("test@example.com"),
            "infra-commit"
        );
        engine.reload(List.of(customRule));

        GithubEventPayload payload = pushEvent("test/repo");
        List<ContextRule> matched = engine.evaluate(payload);
        assertThat(matched).hasSize(1);
        assertThat(matched.get(0).id()).isEqualTo("custom-test");
    }

    @Test
    void pouletsPushEvent_matchesPouletsTemplate() {
        GithubEventPayload payload = pushEvent("fasodigit/poulets");
        List<ContextRule> matched = engine.evaluate(payload);
        assertThat(matched).anyMatch(r -> r.template().equals("poulets-commit"));
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private GithubEventPayload pushEvent(String repoFullName) {
        return new GithubEventPayload(
            "push", "delivery-test",
            new GithubEventPayload.Repository(repoFullName,
                repoFullName.split("/")[1],
                "https://github.com/" + repoFullName, null),
            null, "refs/heads/main", null, List.of(), null
        );
    }

    private GithubEventPayload prEvent(String repoFullName, String state, boolean merged) {
        return new GithubEventPayload(
            "pull_request", "delivery-pr-test",
            new GithubEventPayload.Repository(repoFullName,
                repoFullName.split("/")[1],
                "https://github.com/" + repoFullName, null),
            null, null, null, null,
            new GithubEventPayload.PullRequest(1, "test PR",
                "https://github.com/" + repoFullName + "/pull/1",
                state, merged, null, null,
                new GithubEventPayload.Branch("feature/x", "sha1"),
                new GithubEventPayload.Branch("main", "sha2"))
        );
    }
}
