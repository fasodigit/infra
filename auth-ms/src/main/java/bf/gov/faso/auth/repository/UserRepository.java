package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.User;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
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
}
