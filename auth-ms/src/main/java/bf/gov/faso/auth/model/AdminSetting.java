// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

import jakarta.persistence.*;
import org.hibernate.annotations.JdbcTypeCode;
import org.hibernate.type.SqlTypes;

import java.time.Instant;
import java.util.UUID;

/**
 * Configuration Center setting (JSONB-backed).
 * <p>
 * Optimistic concurrency: callers must pass the current {@code version} to
 * {@code AdminSettingsService.update()}; the service performs a CAS check and
 * increments {@code version} on success, while writing an
 * {@link AdminSettingsHistory} row.
 */
@Entity
@Table(name = "admin_settings")
public class AdminSetting {

    @Id
    @Column(name = "key", length = 120)
    private String key;

    @JdbcTypeCode(SqlTypes.JSON)
    @Column(name = "value", nullable = false, columnDefinition = "JSONB")
    private String value;

    @Column(name = "value_type", nullable = false, length = 20)
    private String valueType;

    @Column(name = "category", nullable = false, length = 40)
    private String category;

    @JdbcTypeCode(SqlTypes.JSON)
    @Column(name = "min_value", columnDefinition = "JSONB")
    private String minValue;

    @JdbcTypeCode(SqlTypes.JSON)
    @Column(name = "max_value", columnDefinition = "JSONB")
    private String maxValue;

    @JdbcTypeCode(SqlTypes.JSON)
    @Column(name = "default_value", nullable = false, columnDefinition = "JSONB")
    private String defaultValue;

    @Column(name = "description", columnDefinition = "TEXT")
    private String description;

    @Column(name = "required_role_to_edit", nullable = false, length = 40)
    private String requiredRoleToEdit = "SUPER_ADMIN";

    @Column(name = "version", nullable = false)
    private long version = 1L;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt = Instant.now();

    @Column(name = "updated_by")
    private UUID updatedBy;

    public AdminSetting() {}

    public String getKey() { return key; }
    public void setKey(String key) { this.key = key; }

    public String getValue() { return value; }
    public void setValue(String value) { this.value = value; }

    public String getValueType() { return valueType; }
    public void setValueType(String valueType) { this.valueType = valueType; }

    public String getCategory() { return category; }
    public void setCategory(String category) { this.category = category; }

    public String getMinValue() { return minValue; }
    public void setMinValue(String minValue) { this.minValue = minValue; }

    public String getMaxValue() { return maxValue; }
    public void setMaxValue(String maxValue) { this.maxValue = maxValue; }

    public String getDefaultValue() { return defaultValue; }
    public void setDefaultValue(String defaultValue) { this.defaultValue = defaultValue; }

    public String getDescription() { return description; }
    public void setDescription(String description) { this.description = description; }

    public String getRequiredRoleToEdit() { return requiredRoleToEdit; }
    public void setRequiredRoleToEdit(String requiredRoleToEdit) { this.requiredRoleToEdit = requiredRoleToEdit; }

    public long getVersion() { return version; }
    public void setVersion(long version) { this.version = version; }

    public Instant getUpdatedAt() { return updatedAt; }
    public void setUpdatedAt(Instant updatedAt) { this.updatedAt = updatedAt; }

    public UUID getUpdatedBy() { return updatedBy; }
    public void setUpdatedBy(UUID updatedBy) { this.updatedBy = updatedBy; }
}
