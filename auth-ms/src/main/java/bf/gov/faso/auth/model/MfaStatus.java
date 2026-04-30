// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

import jakarta.persistence.*;

import java.time.Instant;
import java.util.UUID;

/**
 * Per-user MFA roll-up. Recomputed by {@code AdminMfaEnrollmentService}
 * whenever a TOTP / passkey / recovery-code transition happens, so the admin
 * dashboard can render coverage without joining four tables on every list.
 */
@Entity
@Table(name = "mfa_status")
public class MfaStatus {

    @Id
    @Column(name = "user_id")
    private UUID userId;

    @Column(name = "totp_enabled", nullable = false)
    private boolean totpEnabled = false;

    @Column(name = "passkey_count", nullable = false)
    private int passkeyCount = 0;

    @Column(name = "backup_codes_remaining", nullable = false)
    private int backupCodesRemaining = 0;

    @Column(name = "trusted_devices_count", nullable = false)
    private int trustedDevicesCount = 0;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt = Instant.now();

    public MfaStatus() {}

    public MfaStatus(UUID userId) { this.userId = userId; }

    public UUID getUserId() { return userId; }
    public void setUserId(UUID userId) { this.userId = userId; }

    public boolean isTotpEnabled() { return totpEnabled; }
    public void setTotpEnabled(boolean totpEnabled) { this.totpEnabled = totpEnabled; }

    public int getPasskeyCount() { return passkeyCount; }
    public void setPasskeyCount(int passkeyCount) { this.passkeyCount = passkeyCount; }

    public int getBackupCodesRemaining() { return backupCodesRemaining; }
    public void setBackupCodesRemaining(int backupCodesRemaining) { this.backupCodesRemaining = backupCodesRemaining; }

    public int getTrustedDevicesCount() { return trustedDevicesCount; }
    public void setTrustedDevicesCount(int trustedDevicesCount) { this.trustedDevicesCount = trustedDevicesCount; }

    public Instant getUpdatedAt() { return updatedAt; }
    public void setUpdatedAt(Instant updatedAt) { this.updatedAt = updatedAt; }
}
