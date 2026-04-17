/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.service;

import bf.gov.faso.notifier.domain.GithubEventPayload;
import bf.gov.faso.notifier.domain.NotificationTemplate;
import bf.gov.faso.notifier.metrics.NotifierMetrics;
import com.github.jknack.handlebars.Handlebars;
import com.github.jknack.handlebars.Template;
import com.github.jknack.handlebars.io.ClassPathTemplateLoader;
import com.github.jknack.handlebars.io.TemplateLoader;
import io.micrometer.core.instrument.Timer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.io.IOException;
import java.time.Instant;
import java.util.HashMap;
import java.util.Map;

/**
 * TemplateRenderService — renders Handlebars templates against a GitHub event context.
 *
 * <p>Rendering uses classpath-based templates in {@code /templates/} as the primary source,
 * with DB-persisted templates as overrides (fetched via TemplateResolverService).
 */
@Service
public class TemplateRenderService {

    private static final Logger log = LoggerFactory.getLogger(TemplateRenderService.class);

    private final Handlebars handlebars;
    private final NotifierMetrics metrics;

    public TemplateRenderService(NotifierMetrics metrics) {
        this.metrics = metrics;
        TemplateLoader loader = new ClassPathTemplateLoader("/templates", ".hbs");
        this.handlebars = new Handlebars(loader);
        // Register built-in helpers
        this.handlebars.registerHelpers(HandlebarsHelpers.class);
    }

    /**
     * Render a template by name from classpath.
     */
    public RenderedEmail renderFromClasspath(String templateName, GithubEventPayload payload) {
        Timer.Sample sample = Timer.start();
        try {
            Template subjectTpl = handlebars.compileInline(defaultSubject(templateName, payload));
            Template bodyTpl = handlebars.compile(templateName);

            Map<String, Object> context = buildContext(payload);
            String subject = subjectTpl.apply(context);
            String body = bodyTpl.apply(context);

            return new RenderedEmail(subject, body);
        } catch (IOException e) {
            throw new TemplateNotFoundException(templateName);
        } finally {
            sample.stop(metrics.templateRenderTimer());
        }
    }

    /**
     * Render from a DB-persisted template (body provided as Handlebars source string).
     */
    public RenderedEmail renderFromSource(NotificationTemplate template, GithubEventPayload payload) {
        Timer.Sample sample = Timer.start();
        try {
            Template subjectTpl = handlebars.compileInline(template.getSubjectTemplate());
            Template bodyTpl = handlebars.compileInline(template.getBodyHbs());

            Map<String, Object> context = buildContext(payload);
            String subject = subjectTpl.apply(context);
            String body = bodyTpl.apply(context);

            return new RenderedEmail(subject, body);
        } catch (IOException e) {
            log.error("Failed to render template '{}': {}", template.getName(), e.getMessage());
            throw new RuntimeException("Template render failure: " + template.getName(), e);
        } finally {
            sample.stop(metrics.templateRenderTimer());
        }
    }

    // ── Context builder ───────────────────────────────────────────────────────

    private Map<String, Object> buildContext(GithubEventPayload payload) {
        Map<String, Object> ctx = new HashMap<>();
        ctx.put("event_type", payload.eventType());
        ctx.put("ref", payload.ref());
        ctx.put("compare_url", payload.compareUrl());
        ctx.put("rendered_at", Instant.now().toString());

        if (payload.repository() != null) {
            ctx.put("repo_full_name", payload.repository().fullName());
            ctx.put("repo_name", payload.repository().name());
            ctx.put("repo_url", payload.repository().htmlUrl());
            ctx.put("repo_description", payload.repository().description());
        }

        if (payload.sender() != null) {
            ctx.put("author_login", payload.sender().login());
            ctx.put("avatar_url", payload.sender().avatarUrl());
            ctx.put("author_url", payload.sender().htmlUrl());
        }

        if (payload.commits() != null) {
            ctx.put("commits", payload.commits());
            ctx.put("commit_count", payload.commits().size());
            if (!payload.commits().isEmpty()) {
                ctx.put("first_commit", payload.commits().get(0));
                ctx.put("last_commit", payload.commits().get(payload.commits().size() - 1));
            }
        }

        if (payload.pullRequest() != null) {
            var pr = payload.pullRequest();
            ctx.put("pr_number", pr.number());
            ctx.put("pr_title", pr.title());
            ctx.put("pr_url", pr.htmlUrl());
            ctx.put("pr_state", pr.state());
            ctx.put("pr_merged", pr.merged());
            ctx.put("pr_body", pr.body());
            if (pr.head() != null) ctx.put("pr_head_ref", pr.head().ref());
            if (pr.base() != null) ctx.put("pr_base_ref", pr.base().ref());
            if (pr.user() != null) ctx.put("pr_author", pr.user().login());
        }

        // Branch extraction from ref (refs/heads/main → main)
        if (payload.ref() != null && payload.ref().startsWith("refs/heads/")) {
            ctx.put("branch", payload.ref().substring("refs/heads/".length()));
        }

        return ctx;
    }

    private String defaultSubject(String templateName, GithubEventPayload payload) {
        String repo = payload.repository() != null ? payload.repository().fullName() : "unknown";
        return "[FASO] " + templateName + " — " + repo;
    }

    /** Value type holding rendered subject and HTML body. */
    public record RenderedEmail(String subject, String body) {}
}
