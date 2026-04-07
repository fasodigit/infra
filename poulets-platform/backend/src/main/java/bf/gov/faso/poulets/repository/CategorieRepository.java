package bf.gov.faso.poulets.repository;

import bf.gov.faso.poulets.model.Categorie;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;
import java.util.UUID;

@Repository
public interface CategorieRepository extends JpaRepository<Categorie, UUID> {

    Optional<Categorie> findByName(String name);
}
