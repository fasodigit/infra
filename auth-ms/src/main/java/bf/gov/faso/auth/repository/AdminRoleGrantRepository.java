// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.AdminRoleGrant;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.UUID;

@Repository
public interface AdminRoleGrantRepository extends JpaRepository<AdminRoleGrant, UUID> {

    List<AdminRoleGrant> findByStatus(AdminRoleGrant.Status status);

    List<AdminRoleGrant> findByGranteeIdOrderByCreatedAtDesc(UUID granteeId);

    /**
     * Pending grants visible to a specific approver. Currently — every
     * SUPER-ADMIN can approve any PENDING request, so the {@code approverId}
     * acts as a future namespace filter (e.g. department scoping).
     */
    @Query("SELECT g FROM AdminRoleGrant g WHERE g.status = 'PENDING' " +
           "AND g.grantorId <> :approverId")
    List<AdminRoleGrant> findPendingByApproverId(@Param("approverId") UUID approverId);
}
