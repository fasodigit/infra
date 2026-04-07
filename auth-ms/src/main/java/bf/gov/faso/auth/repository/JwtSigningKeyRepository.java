package bf.gov.faso.auth.repository;

import bf.gov.faso.auth.model.JwtSigningKey;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public interface JwtSigningKeyRepository extends JpaRepository<JwtSigningKey, UUID> {

    Optional<JwtSigningKey> findByKid(String kid);

    List<JwtSigningKey> findByActiveTrue();

    Optional<JwtSigningKey> findFirstByActiveTrueOrderByCreatedAtDesc();
}
