// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

import jakarta.persistence.Column;
import jakarta.persistence.Entity;
import jakarta.persistence.Id;
import jakarta.persistence.Table;
import org.hibernate.annotations.JdbcTypeCode;
import org.hibernate.type.SqlTypes;

import java.time.Instant;

/**
 * Static catalogue row of a fine-grained capability (delta §1).
 * The set is seeded from V10 and remains stable across releases — adding a
 * new capability is a migration + i18n bundle update.
 */
@Entity
@Table(name = "capability_registry")
public class CapabilityRegistry {

    @Id
    @Column(name = "key", length = 80, nullable = false)
    private String key;

    @Column(name = "category", nullable = false, length = 40)
    private String category;

    @Column(name = "description_i18n_key", nullable = false, length = 120)
    private String descriptionI18nKey;

    /**
     * Postgres TEXT[] — Hibernate 6.x maps via {@link SqlTypes#ARRAY}.
     * Subset of {SUPER_ADMIN, ADMIN, MANAGER}.
     */
    @JdbcTypeCode(SqlTypes.ARRAY)
    @Column(name = "applicable_to_roles", columnDefinition = "text[]", nullable = false)
    private String[] applicableToRoles;

    @Column(name = "created_at", nullable = false)
    private Instant createdAt = Instant.now();

    public CapabilityRegistry() {}

    public String getKey() { return key; }
    public void setKey(String key) { this.key = key; }

    public String getCategory() { return category; }
    public void setCategory(String category) { this.category = category; }

    public String getDescriptionI18nKey() { return descriptionI18nKey; }
    public void setDescriptionI18nKey(String descriptionI18nKey) { this.descriptionI18nKey = descriptionI18nKey; }

    public String[] getApplicableToRoles() { return applicableToRoles; }
    public void setApplicableToRoles(String[] applicableToRoles) { this.applicableToRoles = applicableToRoles; }

    public Instant getCreatedAt() { return createdAt; }
    public void setCreatedAt(Instant createdAt) { this.createdAt = createdAt; }
}
