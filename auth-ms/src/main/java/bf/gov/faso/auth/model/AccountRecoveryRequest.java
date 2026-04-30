// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

import jakarta.persistence.*;

import java.time.Instant;
import java.util.UUID;

/**
 * Account recovery request (delta §5).
 *
 * <p>Two flows:
 * <ul>
 *   <li>{@code SELF}            — user lost MFA, magic-link JWT 30 min single-use.</li>
 *   <li>{@code ADMIN_INITIATED} — SUPER_ADMIN reset target's MFA + token 8 chiffres TTL 1h.</li>
 * </ul>
 */
@Entity
@Table(name = "account_recovery_requests")
public class AccountRecoveryRequest {

    public enum Type { SELF, ADMIN_INITIATED }

    public enum Status { PENDING, USED, EXPIRED, REJECTED }

    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    private UUID id;

    @Column(name = "user_id", nullable = false)
    private UUID userId;

    @Column(name = "initiated_by")
    private UUID initiatedBy;

    @Enumerated(EnumType.STRING)
    @Column(name = "recovery_type", nullable = false, length = 20)
    private Type recoveryType;

    @Column(name = "token_hash", nullable = false, length = 255, unique = true)
    private String tokenHash;

    @Column(name = "motif", columnDefinition = "TEXT")
    private String motif;

    @Enumerated(EnumType.STRING)
    @Column(name = "status", nullable = false, length = 20)
    private Status status = Status.PENDING;

    @Column(name = "created_at", nullable = false)
    private Instant createdAt = Instant.now();

    @Column(name = "used_at")
    private Instant usedAt;

    @Column(name = "expires_at", nullable = false)
    private Instant expiresAt;

    @Column(name = "trace_id", length = 32)
    private String traceId;

    public AccountRecoveryRequest() {}

    public UUID getId() { return id; }
    public void setId(UUID id) { this.id = id; }

    public UUID getUserId() { return userId; }
    public void setUserId(UUID userId) { this.userId = userId; }

    public UUID getInitiatedBy() { return initiatedBy; }
    public void setInitiatedBy(UUID initiatedBy) { this.initiatedBy = initiatedBy; }

    public Type getRecoveryType() { return recoveryType; }
    public void setRecoveryType(Type recoveryType) { this.recoveryType = recoveryType; }

    public String getTokenHash() { return tokenHash; }
    public void setTokenHash(String tokenHash) { this.tokenHash = tokenHash; }

    public String getMotif() { return motif; }
    public void setMotif(String motif) { this.motif = motif; }

    public Status getStatus() { return status; }
    public void setStatus(Status status) { this.status = status; }

    public Instant getCreatedAt() { return createdAt; }
    public void setCreatedAt(Instant createdAt) { this.createdAt = createdAt; }

    public Instant getUsedAt() { return usedAt; }
    public void setUsedAt(Instant usedAt) { this.usedAt = usedAt; }

    public Instant getExpiresAt() { return expiresAt; }
    public void setExpiresAt(Instant expiresAt) { this.expiresAt = expiresAt; }

    public String getTraceId() { return traceId; }
    public void setTraceId(String traceId) { this.traceId = traceId; }

    public boolean isExpired() {
        return expiresAt != null && Instant.now().isAfter(expiresAt);
    }
}
