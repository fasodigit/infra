// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.model;

import jakarta.persistence.*;

import java.time.Instant;
import java.util.UUID;

/**
 * Trusted-device registration. Combines:
 * <ul>
 *   <li>{@code fingerprint} — stable hash of UA + IP/24 + Accept-Language</li>
 *   <li>{@code publicKeyPem} — optional WebAuthn-attested device key</li>
 *   <li>lifecycle timestamps {@code createdAt → trustedAt → revokedAt}</li>
 * </ul>
 */
@Entity
@Table(name = "device_registrations")
public class DeviceRegistration {

    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    private UUID id;

    @Column(name = "user_id", nullable = false)
    private UUID userId;

    @Column(name = "fingerprint", nullable = false, length = 128)
    private String fingerprint;

    @Column(name = "device_type", length = 50)
    private String deviceType;

    @Column(name = "public_key_pem", columnDefinition = "TEXT")
    private String publicKeyPem;

    @Column(name = "ua_string", length = 500)
    private String uaString;

    @Column(name = "ip_address", length = 45)
    private String ipAddress;

    @Column(name = "created_at", nullable = false)
    private Instant createdAt = Instant.now();

    @Column(name = "last_used_at")
    private Instant lastUsedAt;

    @Column(name = "trusted_at")
    private Instant trustedAt;

    @Column(name = "revoked_at")
    private Instant revokedAt;

    public DeviceRegistration() {}

    public DeviceRegistration(UUID userId, String fingerprint, String uaString, String ipAddress) {
        this.userId = userId;
        this.fingerprint = fingerprint;
        this.uaString = uaString;
        this.ipAddress = ipAddress;
    }

    public UUID getId() { return id; }
    public void setId(UUID id) { this.id = id; }

    public UUID getUserId() { return userId; }
    public void setUserId(UUID userId) { this.userId = userId; }

    public String getFingerprint() { return fingerprint; }
    public void setFingerprint(String fingerprint) { this.fingerprint = fingerprint; }

    public String getDeviceType() { return deviceType; }
    public void setDeviceType(String deviceType) { this.deviceType = deviceType; }

    public String getPublicKeyPem() { return publicKeyPem; }
    public void setPublicKeyPem(String publicKeyPem) { this.publicKeyPem = publicKeyPem; }

    public String getUaString() { return uaString; }
    public void setUaString(String uaString) { this.uaString = uaString; }

    public String getIpAddress() { return ipAddress; }
    public void setIpAddress(String ipAddress) { this.ipAddress = ipAddress; }

    public Instant getCreatedAt() { return createdAt; }
    public void setCreatedAt(Instant createdAt) { this.createdAt = createdAt; }

    public Instant getLastUsedAt() { return lastUsedAt; }
    public void setLastUsedAt(Instant lastUsedAt) { this.lastUsedAt = lastUsedAt; }

    public Instant getTrustedAt() { return trustedAt; }
    public void setTrustedAt(Instant trustedAt) { this.trustedAt = trustedAt; }

    public Instant getRevokedAt() { return revokedAt; }
    public void setRevokedAt(Instant revokedAt) { this.revokedAt = revokedAt; }

    public boolean isTrusted() {
        return trustedAt != null && revokedAt == null;
    }
}
