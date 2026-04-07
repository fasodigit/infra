package bf.gov.faso.auth;

import bf.gov.faso.auth.model.JwtSigningKey;
import bf.gov.faso.auth.model.Permission;
import bf.gov.faso.auth.model.Role;
import bf.gov.faso.auth.model.User;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Unit tests for auth-ms domain models and logic.
 * These tests do not require Spring context, database, or Redis.
 */
class AuthMsApplicationTests {

    @Test
    @DisplayName("User.isLocked returns true when lockedUntil is in the future")
    void testUserIsLocked() {
        User user = new User();
        user.setLockedUntil(Instant.now().plus(1, ChronoUnit.HOURS));
        assertTrue(user.isLocked());
    }

    @Test
    @DisplayName("User.isLocked returns false when lockedUntil is in the past")
    void testUserIsNotLockedWhenExpired() {
        User user = new User();
        user.setLockedUntil(Instant.now().minus(1, ChronoUnit.HOURS));
        assertFalse(user.isLocked());
    }

    @Test
    @DisplayName("User.isLocked returns true when user is suspended")
    void testUserIsLockedWhenSuspended() {
        User user = new User();
        user.setSuspended(true);
        assertTrue(user.isLocked());
    }

    @Test
    @DisplayName("User.isLocked returns false when no lock and not suspended")
    void testUserIsNotLockedByDefault() {
        User user = new User();
        assertFalse(user.isLocked());
    }

    @Test
    @DisplayName("User.isPasswordExpired returns true when passwordExpiresAt is in the past")
    void testPasswordExpired() {
        User user = new User();
        user.setPasswordExpiresAt(Instant.now().minus(1, ChronoUnit.DAYS));
        assertTrue(user.isPasswordExpired());
    }

    @Test
    @DisplayName("User.isPasswordExpired returns false when passwordExpiresAt is in the future")
    void testPasswordNotExpired() {
        User user = new User();
        user.setPasswordExpiresAt(Instant.now().plus(30, ChronoUnit.DAYS));
        assertFalse(user.isPasswordExpired());
    }

    @Test
    @DisplayName("Permission.toTupleString formats as namespace:object#relation")
    void testPermissionTupleString() {
        Permission perm = new Permission();
        perm.setNamespace("auth");
        perm.setObject("users");
        perm.setRelation("create");

        assertEquals("auth:users#create", perm.toTupleString());
    }

    @Test
    @DisplayName("JwtSigningKey.isExpired returns correct state")
    void testJwtSigningKeyExpired() {
        JwtSigningKey key = new JwtSigningKey();

        key.setExpiresAt(Instant.now().minus(1, ChronoUnit.DAYS));
        assertTrue(key.isExpired());

        key.setExpiresAt(Instant.now().plus(30, ChronoUnit.DAYS));
        assertFalse(key.isExpired());
    }

    @Test
    @DisplayName("User default values are correct")
    void testUserDefaults() {
        User user = new User();
        assertTrue(user.isActive());
        assertFalse(user.isSuspended());
        assertEquals(0, user.getFailedLoginAttempts());
        assertNotNull(user.getRoles());
        assertTrue(user.getRoles().isEmpty());
    }

    @Test
    @DisplayName("Role can hold permissions")
    void testRolePermissions() {
        Role role = new Role();
        role.setName("ADMIN");
        role.setDescription("Administrator");

        Permission perm = new Permission();
        perm.setNamespace("auth");
        perm.setObject("users");
        perm.setRelation("manage");

        role.getPermissions().add(perm);
        assertEquals(1, role.getPermissions().size());
    }

    @Test
    @DisplayName("User can hold multiple roles")
    void testUserRoles() {
        User user = new User();
        user.setEmail("test@faso.gov.bf");
        user.setFirstName("Test");
        user.setLastName("User");

        Role admin = new Role();
        admin.setName("ADMIN");

        Role operator = new Role();
        operator.setName("OPERATOR");

        user.getRoles().add(admin);
        user.getRoles().add(operator);

        assertEquals(2, user.getRoles().size());
    }
}
