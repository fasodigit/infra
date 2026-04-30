// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

import bf.gov.faso.auth.persistence.EncryptedStringConverter;
import jakarta.persistence.*;

import java.time.Instant;
import java.util.UUID;

/**
 * TOTP (RFC 6238) enrolment for a user.
 * <p>
 * The shared secret is stored AES-256-GCM-encrypted via
 * {@link EncryptedStringConverter}. {@code disabled_at != null} means the
 * enrolment has been administratively reset (re-enrolment required).
 */
@Entity
@Table(name = "totp_enrollments")
public class TotpEnrollment {

    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    private UUID id;

    @Column(name = "user_id", nullable = false, unique = true)
    private UUID userId;

    @Column(name = "secret_encrypted", nullable = false, columnDefinition = "TEXT")
    @Convert(converter = EncryptedStringConverter.class)
    private String secretEncrypted;

    @Column(name = "enrolled_at", nullable = false)
    private Instant enrolledAt = Instant.now();

    @Column(name = "disabled_at")
    private Instant disabledAt;

    @Column(name = "last_used_at")
    private Instant lastUsedAt;

    public TotpEnrollment() {}

    public TotpEnrollment(UUID userId, String secretEncrypted) {
        this.userId = userId;
        this.secretEncrypted = secretEncrypted;
    }

    public UUID getId() { return id; }
    public void setId(UUID id) { this.id = id; }

    public UUID getUserId() { return userId; }
    public void setUserId(UUID userId) { this.userId = userId; }

    public String getSecretEncrypted() { return secretEncrypted; }
    public void setSecretEncrypted(String secretEncrypted) { this.secretEncrypted = secretEncrypted; }

    public Instant getEnrolledAt() { return enrolledAt; }
    public void setEnrolledAt(Instant enrolledAt) { this.enrolledAt = enrolledAt; }

    public Instant getDisabledAt() { return disabledAt; }
    public void setDisabledAt(Instant disabledAt) { this.disabledAt = disabledAt; }

    public Instant getLastUsedAt() { return lastUsedAt; }
    public void setLastUsedAt(Instant lastUsedAt) { this.lastUsedAt = lastUsedAt; }

    public boolean isActive() { return disabledAt == null; }
}
