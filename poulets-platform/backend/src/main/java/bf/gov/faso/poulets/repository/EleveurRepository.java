package bf.gov.faso.poulets.repository;

import bf.gov.faso.poulets.model.Eleveur;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface EleveurRepository extends JpaRepository<Eleveur, UUID> {

    Optional<Eleveur> findByUserId(String userId);

    Page<Eleveur> findByActiveTrue(Pageable pageable);

    Page<Eleveur> findByLocationContainingIgnoreCaseAndActiveTrue(String location, Pageable pageable);
}
