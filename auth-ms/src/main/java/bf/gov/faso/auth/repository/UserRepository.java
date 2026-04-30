package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.User;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface UserRepository extends JpaRepository<User, UUID> {

    Optional<User> findByEmail(String email);

    Optional<User> findByKratosIdentityId(String kratosIdentityId);

    boolean existsByEmail(String email);

    Page<User> findAllByActiveTrue(Pageable pageable);

    @Query("SELECT u FROM User u WHERE u.passwordExpiresAt <= :threshold AND u.active = true AND u.suspended = false")
    List<User> findUsersWithExpiringPasswords(@Param("threshold") Instant threshold);

    @Query("SELECT u FROM User u WHERE u.passwordExpiresAt <= :now AND u.active = true")
    List<User> findUsersWithExpiredPasswords(@Param("now") Instant now);

    @Query("SELECT u FROM User u JOIN u.roles r WHERE r.name = :roleName")
    List<User> findByRoleName(@Param("roleName") String roleName);

    @Query("SELECT u FROM User u WHERE u.lockedUntil IS NOT NULL AND u.lockedUntil > :now")
    List<User> findLockedUsers(@Param("now") Instant now);

    /**
     * Phase 4.b.3 — lazy re-hash on login. Updates hash_algo / hash_params /
     * hash_pepper_version columns added by V13 without touching the JPA entity
     * (entity remap is deferred to the stream that owns auth-ms-managed
     * passwords). Returns the number of rows affected (0 or 1).
     *
     * <p>{@code password_hash} is currently NULL on every row (Kratos owns
     * the credential) — callers in 4.b.3 only persist metadata; once auth-ms
     * holds the hash too, the column will be populated as well.
     */
    @Modifying
    @Query(value = "UPDATE users SET hash_algo = :algo, hash_params = CAST(:params AS jsonb), " +
                   "hash_pepper_version = :pepperVersion " +
                   "WHERE id = :userId", nativeQuery = true)
    int updatePasswordHashColumns(@Param("userId") UUID userId,
                                  @Param("passwordHash") String passwordHash,
                                  @Param("algo") String algo,
                                  @Param("params") String params,
                                  @Param("pepperVersion") int pepperVersion);
}
