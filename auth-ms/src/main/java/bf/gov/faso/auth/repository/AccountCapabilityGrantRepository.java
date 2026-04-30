// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.AccountCapabilityGrant;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.UUID;

@Repository
public interface AccountCapabilityGrantRepository extends JpaRepository<AccountCapabilityGrant, UUID> {

    @Query("SELECT g FROM AccountCapabilityGrant g " +
           "WHERE g.userId = :userId AND g.revokedAt IS NULL")
    List<AccountCapabilityGrant> findActiveByUserId(@Param("userId") UUID userId);

    @Query("SELECT g FROM AccountCapabilityGrant g " +
           "WHERE g.userId = :userId AND g.capabilityKey = :capabilityKey " +
           "AND g.revokedAt IS NULL")
    List<AccountCapabilityGrant> findActiveByUserAndKey(@Param("userId") UUID userId,
                                                        @Param("capabilityKey") String capabilityKey);

    /**
     * Native helper used by {@code CapabilityService.checkUniqueness}. Returns
     * the userIds whose ACTIVE capability set is exactly equal to the provided
     * set (same cardinality + same elements). Filtered by role at the service
     * layer (we don't join {@code user_roles} here to keep the query simple).
     *
     * <p>Implementation: the service issues two queries
     *   1) {@code findUsersWithCapability} for the smallest cap →
     *      candidate set;
     *   2) {@code findActiveByUserId} for each candidate to verify exact
     *      equality with the provided set.
     */
    @Query("SELECT DISTINCT g.userId FROM AccountCapabilityGrant g " +
           "WHERE g.capabilityKey = :capabilityKey AND g.revokedAt IS NULL")
    List<UUID> findUsersWithActiveCapability(@Param("capabilityKey") String capabilityKey);
}
