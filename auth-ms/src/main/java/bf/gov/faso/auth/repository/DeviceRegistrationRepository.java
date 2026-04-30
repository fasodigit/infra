// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.DeviceRegistration;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface DeviceRegistrationRepository extends JpaRepository<DeviceRegistration, UUID> {

    Optional<DeviceRegistration> findByUserIdAndFingerprint(UUID userId, String fingerprint);

    List<DeviceRegistration> findByUserIdAndRevokedAtIsNull(UUID userId);

    long countByUserIdAndRevokedAtIsNull(UUID userId);

    long countByUserIdAndTrustedAtIsNotNullAndRevokedAtIsNull(UUID userId);
}
