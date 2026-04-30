// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.TotpEnrollment;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface TotpEnrollmentRepository extends JpaRepository<TotpEnrollment, UUID> {

    Optional<TotpEnrollment> findByUserId(UUID userId);

    Optional<TotpEnrollment> findByUserIdAndDisabledAtIsNull(UUID userId);
}
