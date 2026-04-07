package bf.gov.faso.poulets.service;

import bf.gov.faso.poulets.cache.PouletCacheService;
import bf.gov.faso.poulets.model.Categorie;
import bf.gov.faso.poulets.model.Eleveur;
import bf.gov.faso.poulets.model.Poulet;
import bf.gov.faso.poulets.model.Race;
import bf.gov.faso.poulets.repository.CategorieRepository;
import bf.gov.faso.poulets.repository.EleveurRepository;
import bf.gov.faso.poulets.repository.PouletRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.data.jpa.domain.Specification;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.Map;
import java.util.Optional;
import java.util.UUID;

@Service
public class PouletService {

    private static final Logger log = LoggerFactory.getLogger(PouletService.class);

    private final PouletRepository pouletRepository;
    private final EleveurRepository eleveurRepository;
    private final CategorieRepository categorieRepository;
    private final PouletCacheService cacheService;

    public PouletService(PouletRepository pouletRepository,
                         EleveurRepository eleveurRepository,
                         CategorieRepository categorieRepository,
                         PouletCacheService cacheService) {
        this.pouletRepository = pouletRepository;
        this.eleveurRepository = eleveurRepository;
        this.categorieRepository = categorieRepository;
        this.cacheService = cacheService;
    }

    public Optional<Poulet> findById(UUID id) {
        // Try cache first
        Optional<Poulet> cached = cacheService.getCached(id);
        if (cached.isPresent()) {
            return cached;
        }

        Optional<Poulet> fromDb = pouletRepository.findById(id);
        fromDb.ifPresent(cacheService::cacheFromDb);
        return fromDb;
    }

    public Page<Poulet> findAll(Map<String, Object> filter, int page, int size) {
        PageRequest pageRequest = PageRequest.of(page, size, Sort.by(Sort.Direction.DESC, "createdAt"));

        if (filter == null || filter.isEmpty()) {
            return pouletRepository.findByAvailableTrue(pageRequest);
        }

        Specification<Poulet> spec = Specification.where(null);

        if (filter.containsKey("race")) {
            Race race = Race.valueOf((String) filter.get("race"));
            spec = spec.and((root, query, cb) -> cb.equal(root.get("race"), race));
        }

        if (filter.containsKey("minPrice")) {
            double minPrice = ((Number) filter.get("minPrice")).doubleValue();
            spec = spec.and((root, query, cb) -> cb.greaterThanOrEqualTo(root.get("price"), minPrice));
        }

        if (filter.containsKey("maxPrice")) {
            double maxPrice = ((Number) filter.get("maxPrice")).doubleValue();
            spec = spec.and((root, query, cb) -> cb.lessThanOrEqualTo(root.get("price"), maxPrice));
        }

        if (filter.containsKey("location")) {
            String location = (String) filter.get("location");
            spec = spec.and((root, query, cb) ->
                    cb.like(cb.lower(root.get("eleveur").get("location")),
                            "%" + location.toLowerCase() + "%"));
        }

        if (filter.containsKey("available")) {
            boolean available = (Boolean) filter.get("available");
            spec = spec.and((root, query, cb) -> cb.equal(root.get("available"), available));
        } else {
            // Default: only show available poulets
            spec = spec.and((root, query, cb) -> cb.isTrue(root.get("available")));
        }

        if (filter.containsKey("categorieId")) {
            UUID categorieId = UUID.fromString((String) filter.get("categorieId"));
            spec = spec.and((root, query, cb) -> cb.equal(root.get("categorie").get("id"), categorieId));
        }

        if (filter.containsKey("eleveurId")) {
            UUID eleveurId = UUID.fromString((String) filter.get("eleveurId"));
            spec = spec.and((root, query, cb) -> cb.equal(root.get("eleveur").get("id"), eleveurId));
        }

        return pouletRepository.findAll(spec, pageRequest);
    }

    @Transactional
    public Poulet add(UUID eleveurId, Race race, double weight, double price,
                      int quantity, String description, UUID categorieId) {
        Eleveur eleveur = eleveurRepository.findById(eleveurId)
                .orElseThrow(() -> new IllegalArgumentException("Eleveur not found: " + eleveurId));

        Poulet poulet = new Poulet();
        poulet.setEleveur(eleveur);
        poulet.setRace(race);
        poulet.setWeight(weight);
        poulet.setPrice(price);
        poulet.setQuantity(quantity);
        poulet.setDescription(description);
        poulet.setAvailable(quantity > 0);

        if (categorieId != null) {
            Categorie categorie = categorieRepository.findById(categorieId).orElse(null);
            poulet.setCategorie(categorie);
        }

        Poulet saved = pouletRepository.save(poulet);

        // Write-behind: cache the new poulet and mark dirty
        cacheService.cacheAndMarkDirty(saved);

        log.info("Added poulet: id={} eleveurId={} race={} qty={}", saved.getId(), eleveurId, race, quantity);
        return saved;
    }

    @Transactional
    public Poulet update(UUID id, Race race, Double weight, Double price,
                         Integer quantity, String description, UUID categorieId) {
        Poulet poulet = pouletRepository.findById(id)
                .orElseThrow(() -> new IllegalArgumentException("Poulet not found: " + id));

        if (race != null) poulet.setRace(race);
        if (weight != null) poulet.setWeight(weight);
        if (price != null) poulet.setPrice(price);
        if (quantity != null) {
            poulet.setQuantity(quantity);
            poulet.setAvailable(quantity > 0);
        }
        if (description != null) poulet.setDescription(description);
        if (categorieId != null) {
            Categorie categorie = categorieRepository.findById(categorieId).orElse(null);
            poulet.setCategorie(categorie);
        }

        Poulet saved = pouletRepository.save(poulet);

        // Write-behind: update cache and mark dirty
        cacheService.cacheAndMarkDirty(saved);

        log.info("Updated poulet: id={}", id);
        return saved;
    }

    @Transactional
    public boolean delete(UUID id) {
        Poulet poulet = pouletRepository.findById(id)
                .orElseThrow(() -> new IllegalArgumentException("Poulet not found: " + id));

        poulet.setAvailable(false);
        poulet.setQuantity(0);
        pouletRepository.save(poulet);

        // Invalidate cache
        cacheService.invalidate(id);

        log.info("Soft-deleted poulet: id={}", id);
        return true;
    }
}
