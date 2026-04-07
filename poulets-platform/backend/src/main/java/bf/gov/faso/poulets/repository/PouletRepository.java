package bf.gov.faso.poulets.repository;

import bf.gov.faso.poulets.model.Poulet;
import bf.gov.faso.poulets.model.Race;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.UUID;

@Repository
public interface PouletRepository extends JpaRepository<Poulet, UUID>, JpaSpecificationExecutor<Poulet> {

    Page<Poulet> findByAvailableTrue(Pageable pageable);

    Page<Poulet> findByRaceAndAvailableTrue(Race race, Pageable pageable);

    @Query("SELECT p FROM Poulet p WHERE p.eleveur.id = :eleveurId AND p.available = true")
    List<Poulet> findAvailableByEleveurId(@Param("eleveurId") UUID eleveurId);

    Page<Poulet> findByEleveurId(UUID eleveurId, Pageable pageable);
}
