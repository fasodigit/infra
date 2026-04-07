package bf.gov.faso.poulets.repository;

import bf.gov.faso.poulets.model.CommandeItem;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.UUID;

@Repository
public interface CommandeItemRepository extends JpaRepository<CommandeItem, UUID> {

    List<CommandeItem> findByCommandeId(UUID commandeId);
}
