package bf.gov.faso.poulets.repository;

import bf.gov.faso.poulets.model.Commande;
import bf.gov.faso.poulets.model.CommandeStatus;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.UUID;

@Repository
public interface CommandeRepository extends JpaRepository<Commande, UUID> {

    Page<Commande> findByClientId(UUID clientId, Pageable pageable);

    Page<Commande> findByClientIdAndStatus(UUID clientId, CommandeStatus status, Pageable pageable);

    Page<Commande> findByEleveurId(UUID eleveurId, Pageable pageable);

    Page<Commande> findByEleveurIdAndStatus(UUID eleveurId, CommandeStatus status, Pageable pageable);
}
