// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

import jakarta.persistence.*;

import java.time.Instant;
import java.util.UUID;

/**
 * Single-use MFA recovery code.
 * <p>
 * {@code codeHash} is {@code Argon2id(HMAC-SHA256(pepper, plain))} since
 * Phase 4.b.3 (was {@code bcrypt} in Phase 4.b.1/.2). The plain code is shown
 * ONCE at generation and never re-displayed. {@code usedAt != null} marks the
 * code as consumed.
 */
@Entity
@Table(name = "recovery_codes")
public class RecoveryCode {

    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    private UUID id;

    @Column(name = "user_id", nullable = false)
    private UUID userId;

    @Column(name = "code_hash", nullable = false, length = 120)
    private String codeHash;

    @Column(name = "motif")
    private String motif;

    @Column(name = "generated_at", nullable = false)
    private Instant generatedAt = Instant.now();

    @Column(name = "used_at")
    private Instant usedAt;

    @Column(name = "expires_at", nullable = false)
    private Instant expiresAt;

    public RecoveryCode() {}

    public RecoveryCode(UUID userId, String codeHash, String motif, Instant expiresAt) {
        this.userId = userId;
        this.codeHash = codeHash;
        this.motif = motif;
        this.expiresAt = expiresAt;
    }

    public UUID getId() { return id; }
    public void setId(UUID id) { this.id = id; }

    public UUID getUserId() { return userId; }
    public void setUserId(UUID userId) { this.userId = userId; }

    public String getCodeHash() { return codeHash; }
    public void setCodeHash(String codeHash) { this.codeHash = codeHash; }

    public String getMotif() { return motif; }
    public void setMotif(String motif) { this.motif = motif; }

    public Instant getGeneratedAt() { return generatedAt; }
    public void setGeneratedAt(Instant generatedAt) { this.generatedAt = generatedAt; }

    public Instant getUsedAt() { return usedAt; }
    public void setUsedAt(Instant usedAt) { this.usedAt = usedAt; }

    public Instant getExpiresAt() { return expiresAt; }
    public void setExpiresAt(Instant expiresAt) { this.expiresAt = expiresAt; }

    public boolean isUsed() { return usedAt != null; }
    public boolean isExpired() { return expiresAt != null && Instant.now().isAfter(expiresAt); }
}
