package bf.gov.faso.auth.model;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.HashSet;
import java.util.Set;
import java.util.UUID;

@Entity
@Table(name = "users")
public class User {

    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    private UUID id;

    @Column(nullable = false, unique = true)
    private String email;

    @Column(name = "first_name", nullable = false)
    private String firstName;

    @Column(name = "last_name", nullable = false)
    private String lastName;

    private String department;

    @Column(name = "phone_number")
    private String phoneNumber;

    @Column(nullable = false)
    private boolean active = true;

    @Column(name = "kratos_identity_id", unique = true)
    private String kratosIdentityId;

    @Column(name = "password_changed_at", nullable = false)
    private Instant passwordChangedAt = Instant.now();

    @Column(name = "password_expires_at", nullable = false)
    private Instant passwordExpiresAt;

    @Column(name = "locked_until")
    private Instant lockedUntil;

    @Column(name = "failed_login_attempts", nullable = false)
    private int failedLoginAttempts = 0;

    @Column(nullable = false)
    private boolean suspended = false;

    /**
     * Set true after a recovery flow (self or admin-initiated). The user MUST
     * re-enrol MFA (TOTP or PassKey) before any other privileged action.
     * Reset to false once a fresh enrolment succeeds.
     */
    @Column(name = "must_reenroll_mfa", nullable = false)
    private boolean mustReenrollMfa = false;

    @ManyToMany(fetch = FetchType.LAZY)
    @JoinTable(
        name = "user_roles",
        joinColumns = @JoinColumn(name = "user_id"),
        inverseJoinColumns = @JoinColumn(name = "role_id")
    )
    private Set<Role> roles = new HashSet<>();

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt = Instant.now();

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt = Instant.now();

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
        updatedAt = Instant.now();
        if (passwordExpiresAt == null) {
            passwordExpiresAt = Instant.now().plusSeconds(90L * 24 * 60 * 60);
        }
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }

    // --- Getters and Setters ---

    public UUID getId() { return id; }
    public void setId(UUID id) { this.id = id; }

    public String getEmail() { return email; }
    public void setEmail(String email) { this.email = email; }

    public String getFirstName() { return firstName; }
    public void setFirstName(String firstName) { this.firstName = firstName; }

    public String getLastName() { return lastName; }
    public void setLastName(String lastName) { this.lastName = lastName; }

    public String getDepartment() { return department; }
    public void setDepartment(String department) { this.department = department; }

    public String getPhoneNumber() { return phoneNumber; }
    public void setPhoneNumber(String phoneNumber) { this.phoneNumber = phoneNumber; }

    public boolean isActive() { return active; }
    public void setActive(boolean active) { this.active = active; }

    public String getKratosIdentityId() { return kratosIdentityId; }
    public void setKratosIdentityId(String kratosIdentityId) { this.kratosIdentityId = kratosIdentityId; }

    public Instant getPasswordChangedAt() { return passwordChangedAt; }
    public void setPasswordChangedAt(Instant passwordChangedAt) { this.passwordChangedAt = passwordChangedAt; }

    public Instant getPasswordExpiresAt() { return passwordExpiresAt; }
    public void setPasswordExpiresAt(Instant passwordExpiresAt) { this.passwordExpiresAt = passwordExpiresAt; }

    public Instant getLockedUntil() { return lockedUntil; }
    public void setLockedUntil(Instant lockedUntil) { this.lockedUntil = lockedUntil; }

    public int getFailedLoginAttempts() { return failedLoginAttempts; }
    public void setFailedLoginAttempts(int failedLoginAttempts) { this.failedLoginAttempts = failedLoginAttempts; }

    public boolean isSuspended() { return suspended; }
    public void setSuspended(boolean suspended) { this.suspended = suspended; }

    public boolean isMustReenrollMfa() { return mustReenrollMfa; }
    public void setMustReenrollMfa(boolean mustReenrollMfa) { this.mustReenrollMfa = mustReenrollMfa; }

    public Set<Role> getRoles() { return roles; }
    public void setRoles(Set<Role> roles) { this.roles = roles; }

    public Instant getCreatedAt() { return createdAt; }
    public Instant getUpdatedAt() { return updatedAt; }

    public boolean isLocked() {
        if (suspended) return true;
        return lockedUntil != null && Instant.now().isBefore(lockedUntil);
    }

    public boolean isPasswordExpired() {
        return passwordExpiresAt != null && Instant.now().isAfter(passwordExpiresAt);
    }
}
