/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier;

import bf.gov.faso.notifier.domain.GithubEventPayload;
import bf.gov.faso.notifier.metrics.NotifierMetrics;
import bf.gov.faso.notifier.service.TemplateNotFoundException;
import bf.gov.faso.notifier.service.TemplateRenderService;
import io.micrometer.core.instrument.simple.SimpleMeterRegistry;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * TemplateRenderTest — unit tests for Handlebars template rendering.
 */
class TemplateRenderTest {

    private TemplateRenderService renderService;

    @BeforeEach
    void setUp() {
        renderService = new TemplateRenderService(new NotifierMetrics(new SimpleMeterRegistry()));
    }

    @ParameterizedTest
    @ValueSource(strings = {
        "infra-commit",
        "vouchers-commit",
        "etatcivil-commit",
        "poulets-commit",
        "sogesy-commit",
        "hospital-commit",
        "escool-commit",
        "eticket-commit",
        "altmission-commit",
        "fasokalan-commit",
        "pull-request-opened",
        "pull-request-merged"
    })
    void allTemplatesRenderWithoutError(String templateName) {
        GithubEventPayload payload = buildFullPayload();
        TemplateRenderService.RenderedEmail result =
            renderService.renderFromClasspath(templateName, payload);

        assertThat(result.body()).isNotBlank();
        assertThat(result.body()).contains("fasodigit/infra");
        assertThat(result.subject()).isNotBlank();
    }

    @Test
    void infraCommitTemplate_containsCommitShas() {
        GithubEventPayload payload = buildFullPayload();
        TemplateRenderService.RenderedEmail result =
            renderService.renderFromClasspath("infra-commit", payload);

        assertThat(result.body()).contains("abc1234"); // shortSha of abc1234567890
        assertThat(result.body()).contains("feat(kaya)");
        assertThat(result.body()).contains("main"); // branch from refs/heads/main
    }

    @Test
    void pullRequestTemplate_containsPRDetails() {
        GithubEventPayload payload = buildPRPayload();
        TemplateRenderService.RenderedEmail result =
            renderService.renderFromClasspath("pull-request-opened", payload);

        assertThat(result.body()).contains("#42");
        assertThat(result.body()).contains("feat: add bulk order");
        assertThat(result.body()).contains("feature/bulk-order");
    }

    @Test
    void unknownTemplate_throwsTemplateNotFoundException() {
        GithubEventPayload payload = buildFullPayload();
        assertThatThrownBy(() ->
            renderService.renderFromClasspath("nonexistent-template", payload))
            .isInstanceOf(TemplateNotFoundException.class);
    }

    @Test
    void renderWithNullPayload_gracefullyHandlesNulls() {
        GithubEventPayload minimalPayload = new GithubEventPayload(
            "push", "delivery-123",
            new GithubEventPayload.Repository("fasodigit/infra", "infra",
                "https://github.com/fasodigit/infra", null),
            null, "refs/heads/main", null, List.of(), null
        );
        TemplateRenderService.RenderedEmail result =
            renderService.renderFromClasspath("infra-commit", minimalPayload);
        assertThat(result.body()).isNotBlank();
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private GithubEventPayload buildFullPayload() {
        return new GithubEventPayload(
            "push", "delivery-abc",
            new GithubEventPayload.Repository("fasodigit/infra", "infra",
                "https://github.com/fasodigit/infra", "FASO Infrastructure"),
            new GithubEventPayload.Sender("devops-bot",
                "https://avatars.githubusercontent.com/u/1?v=4",
                "https://github.com/devops-bot"),
            "refs/heads/main",
            "https://github.com/fasodigit/infra/compare/old...new",
            List.of(
                new GithubEventPayload.Commit("abc1234567890", "feat(kaya): add WAL persistence",
                    new GithubEventPayload.CommitAuthor("DevOps", "devops@faso.gov.bf"),
                    "https://github.com/fasodigit/infra/commit/abc1234",
                    List.of("kaya/wal.rs"), List.of("kaya/Cargo.toml"), List.of()),
                new GithubEventPayload.Commit("def5678901234", "fix(armageddon): increase timeout",
                    new GithubEventPayload.CommitAuthor("SRE", "sre@faso.gov.bf"),
                    "https://github.com/fasodigit/infra/commit/def5678",
                    List.of(), List.of("armageddon/config.toml"), List.of("old.toml"))
            ),
            null
        );
    }

    private GithubEventPayload buildPRPayload() {
        return new GithubEventPayload(
            "pull_request", "delivery-pr",
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
    }
}
