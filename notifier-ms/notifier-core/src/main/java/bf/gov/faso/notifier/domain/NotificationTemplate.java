/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.domain;

import jakarta.persistence.*;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.Size;
import org.hibernate.annotations.JdbcTypeCode;
import org.hibernate.type.SqlTypes;

import java.time.Instant;

/**
 * NotificationTemplate — persisted Handlebars template definition.
 *
 * <p>{@code body_hbs} is the full HTML Handlebars source; {@code context_rules_json}
 * encodes routing logic in JSON-Logic format. Templates are resolved by name
 * at dispatch time and rendered with the GitHub event payload as context.
 */
@Entity
@Table(name = "notification_templates",
       uniqueConstraints = @UniqueConstraint(name = "uk_template_name", columnNames = "name"))
public class NotificationTemplate {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @NotBlank
    @Size(max = 128)
    @Column(name = "name", nullable = false, unique = true, length = 128)
    private String name;

    @NotBlank
    @Size(max = 512)
    @Column(name = "subject_template", nullable = false, length = 512)
    private String subjectTemplate;

    @NotBlank
    @Column(name = "body_hbs", nullable = false, columnDefinition = "TEXT")
    private String bodyHbs;

    /** JSON-Logic rules controlling when this template applies. */
    @JdbcTypeCode(SqlTypes.JSON)
    @Column(name = "context_rules_json", columnDefinition = "jsonb")
    private String contextRulesJson;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
        updatedAt = createdAt;
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }

    // ── Getters / Setters ──────────────────────────────────────────

    public Long getId() { return id; }

    public String getName() { return name; }
    public void setName(String name) { this.name = name; }

    public String getSubjectTemplate() { return subjectTemplate; }
    public void setSubjectTemplate(String subjectTemplate) { this.subjectTemplate = subjectTemplate; }

    public String getBodyHbs() { return bodyHbs; }
    public void setBodyHbs(String bodyHbs) { this.bodyHbs = bodyHbs; }

    public String getContextRulesJson() { return contextRulesJson; }
    public void setContextRulesJson(String contextRulesJson) { this.contextRulesJson = contextRulesJson; }

    public Instant getCreatedAt() { return createdAt; }
    public Instant getUpdatedAt() { return updatedAt; }
}
