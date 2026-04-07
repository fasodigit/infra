package bf.gov.faso.poulets.service;

import bf.gov.faso.poulets.model.Eleveur;
import bf.gov.faso.poulets.repository.EleveurRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.Optional;
import java.util.UUID;

@Service
public class EleveurService {

    private static final Logger log = LoggerFactory.getLogger(EleveurService.class);

    private final EleveurRepository eleveurRepository;

    public EleveurService(EleveurRepository eleveurRepository) {
        this.eleveurRepository = eleveurRepository;
    }

    public Optional<Eleveur> findById(UUID id) {
        return eleveurRepository.findById(id);
    }

    public Optional<Eleveur> findByUserId(String userId) {
        return eleveurRepository.findByUserId(userId);
    }

    public Page<Eleveur> findAll(String location, int page, int size) {
        PageRequest pageRequest = PageRequest.of(page, size, Sort.by(Sort.Direction.DESC, "createdAt"));
        if (location != null && !location.isBlank()) {
            return eleveurRepository.findByLocationContainingIgnoreCaseAndActiveTrue(location, pageRequest);
        }
        return eleveurRepository.findByActiveTrue(pageRequest);
    }

    @Transactional
    public Eleveur register(String userId, String name, String phone, String location, String description) {
        Eleveur eleveur = new Eleveur();
        eleveur.setUserId(userId);
        eleveur.setName(name);
        eleveur.setPhone(phone);
        eleveur.setLocation(location);
        eleveur.setDescription(description);

        Eleveur saved = eleveurRepository.save(eleveur);
        log.info("Registered eleveur: id={} name={} location={}", saved.getId(), name, location);
        return saved;
    }

    @Transactional
    public Eleveur update(UUID id, String name, String phone, String location, String description) {
        Eleveur eleveur = eleveurRepository.findById(id)
                .orElseThrow(() -> new IllegalArgumentException("Eleveur not found: " + id));

        if (name != null) eleveur.setName(name);
        if (phone != null) eleveur.setPhone(phone);
        if (location != null) eleveur.setLocation(location);
        if (description != null) eleveur.setDescription(description);

        Eleveur saved = eleveurRepository.save(eleveur);
        log.info("Updated eleveur: id={}", id);
        return saved;
    }
}
