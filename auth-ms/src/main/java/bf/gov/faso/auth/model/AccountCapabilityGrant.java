// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

import jakarta.persistence.*;

import java.time.Instant;
import java.util.UUID;

/**
 * Per-user capability grant — append-only record. An "active" grant has
 * {@code revokedAt == null}. Two ADMIN (or two MANAGER) accounts MUST NOT
 * share the exact same set of active capabilities (soft uniqueness, see
 * {@code CapabilityService.checkUniqueness}).
 */
@Entity
@Table(name = "account_capability_grants")
public class AccountCapabilityGrant {

    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    private UUID id;

    @Column(name = "user_id", nullable = false)
    private UUID userId;

    @Column(name = "capability_key", nullable = false, length = 80)
    private String capabilityKey;

    @Column(name = "scope", columnDefinition = "jsonb")
    private String scopeJson;

    @Column(name = "granted_by")
    private UUID grantedBy;

    @Column(name = "granted_at", nullable = false)
    private Instant grantedAt = Instant.now();

    @Column(name = "revoked_at")
    private Instant revokedAt;

    @Column(name = "revoked_by")
    private UUID revokedBy;

    @Column(name = "granted_for_role", length = 20)
    private String grantedForRole;

    @Column(name = "motif", columnDefinition = "TEXT")
    private String motif;

    public AccountCapabilityGrant() {}

    public UUID getId() { return id; }
    public void setId(UUID id) { this.id = id; }

    public UUID getUserId() { return userId; }
    public void setUserId(UUID userId) { this.userId = userId; }

    public String getCapabilityKey() { return capabilityKey; }
    public void setCapabilityKey(String capabilityKey) { this.capabilityKey = capabilityKey; }

    public String getScopeJson() { return scopeJson; }
    public void setScopeJson(String scopeJson) { this.scopeJson = scopeJson; }

    public UUID getGrantedBy() { return grantedBy; }
    public void setGrantedBy(UUID grantedBy) { this.grantedBy = grantedBy; }

    public Instant getGrantedAt() { return grantedAt; }
    public void setGrantedAt(Instant grantedAt) { this.grantedAt = grantedAt; }

    public Instant getRevokedAt() { return revokedAt; }
    public void setRevokedAt(Instant revokedAt) { this.revokedAt = revokedAt; }

    public UUID getRevokedBy() { return revokedBy; }
    public void setRevokedBy(UUID revokedBy) { this.revokedBy = revokedBy; }

    public String getGrantedForRole() { return grantedForRole; }
    public void setGrantedForRole(String grantedForRole) { this.grantedForRole = grantedForRole; }

    public String getMotif() { return motif; }
    public void setMotif(String motif) { this.motif = motif; }

    public boolean isActive() { return revokedAt == null; }
}
