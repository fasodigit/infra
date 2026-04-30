// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.CapabilityRegistry;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;

@Repository
public interface CapabilityRegistryRepository extends JpaRepository<CapabilityRegistry, String> {

    List<CapabilityRegistry> findByCategory(String category);
}
