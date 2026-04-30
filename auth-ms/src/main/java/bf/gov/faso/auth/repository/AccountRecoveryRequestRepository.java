// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.AccountRecoveryRequest;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface AccountRecoveryRequestRepository extends JpaRepository<AccountRecoveryRequest, UUID> {

    Optional<AccountRecoveryRequest> findByTokenHash(String tokenHash);

    List<AccountRecoveryRequest> findByUserIdOrderByCreatedAtDesc(UUID userId);

    List<AccountRecoveryRequest> findByUserIdAndStatus(UUID userId, AccountRecoveryRequest.Status status);
}
