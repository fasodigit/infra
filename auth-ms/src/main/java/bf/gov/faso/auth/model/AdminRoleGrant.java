// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

import jakarta.persistence.*;

import java.time.Instant;
import java.util.UUID;

/**
 * Dual-control role grant request.
 * State machine: PENDING -> APPROVED | REJECTED | EXPIRED.
 */
@Entity
@Table(name = "admin_role_grants")
public class AdminRoleGrant {

    public enum Status { PENDING, APPROVED, REJECTED, EXPIRED }

    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    private UUID id;

    @Column(name = "grantor_id", nullable = false)
    private UUID grantorId;

    @Column(name = "grantee_id", nullable = false)
    private UUID granteeId;

    @Column(name = "role_id", nullable = false)
    private UUID roleId;

    @Column(name = "justification", nullable = false, columnDefinition = "TEXT")
    private String justification;

    @Enumerated(EnumType.STRING)
    @Column(name = "status", nullable = false, length = 20)
    private Status status = Status.PENDING;

    @Column(name = "approver_id")
    private UUID approverId;

    @Column(name = "expires_at")
    private Instant expiresAt;

    @Column(name = "created_at", nullable = false)
    private Instant createdAt = Instant.now();

    @Column(name = "approved_at")
    private Instant approvedAt;

    @Column(name = "rejected_at")
    private Instant rejectedAt;

    @Column(name = "rejection_reason", columnDefinition = "TEXT")
    private String rejectionReason;

    public AdminRoleGrant() {}

    public UUID getId() { return id; }
    public void setId(UUID id) { this.id = id; }

    public UUID getGrantorId() { return grantorId; }
    public void setGrantorId(UUID grantorId) { this.grantorId = grantorId; }

    public UUID getGranteeId() { return granteeId; }
    public void setGranteeId(UUID granteeId) { this.granteeId = granteeId; }

    public UUID getRoleId() { return roleId; }
    public void setRoleId(UUID roleId) { this.roleId = roleId; }

    public String getJustification() { return justification; }
    public void setJustification(String justification) { this.justification = justification; }

    public Status getStatus() { return status; }
    public void setStatus(Status status) { this.status = status; }

    public UUID getApproverId() { return approverId; }
    public void setApproverId(UUID approverId) { this.approverId = approverId; }

    public Instant getExpiresAt() { return expiresAt; }
    public void setExpiresAt(Instant expiresAt) { this.expiresAt = expiresAt; }

    public Instant getCreatedAt() { return createdAt; }
    public void setCreatedAt(Instant createdAt) { this.createdAt = createdAt; }

    public Instant getApprovedAt() { return approvedAt; }
    public void setApprovedAt(Instant approvedAt) { this.approvedAt = approvedAt; }

    public Instant getRejectedAt() { return rejectedAt; }
    public void setRejectedAt(Instant rejectedAt) { this.rejectedAt = rejectedAt; }

    public String getRejectionReason() { return rejectionReason; }
    public void setRejectionReason(String rejectionReason) { this.rejectionReason = rejectionReason; }
}
