// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.AdminSettingsHistory;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;

@Repository
public interface AdminSettingsHistoryRepository extends JpaRepository<AdminSettingsHistory, Long> {

    List<AdminSettingsHistory> findByKeyOrderByVersionDesc(String key);

    Optional<AdminSettingsHistory> findByKeyAndVersion(String key, long version);
}
