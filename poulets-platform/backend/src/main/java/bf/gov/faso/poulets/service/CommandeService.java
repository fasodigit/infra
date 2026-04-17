package bf.gov.faso.poulets.service;

import bf.gov.faso.poulets.model.*;
import bf.gov.faso.poulets.repository.*;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.dao.OptimisticLockingFailureException;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Sort;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

@Service
public class CommandeService {

    private static final Logger log = LoggerFactory.getLogger(CommandeService.class);

    private final CommandeRepository commandeRepository;
    private final CommandeItemRepository commandeItemRepository;
    private final ClientRepository clientRepository;
    private final EleveurRepository eleveurRepository;
    private final PouletRepository pouletRepository;

    public CommandeService(CommandeRepository commandeRepository,
                           CommandeItemRepository commandeItemRepository,
                           ClientRepository clientRepository,
                           EleveurRepository eleveurRepository,
                           PouletRepository pouletRepository) {
        this.commandeRepository = commandeRepository;
        this.commandeItemRepository = commandeItemRepository;
        this.clientRepository = clientRepository;
        this.eleveurRepository = eleveurRepository;
        this.pouletRepository = pouletRepository;
    }

    public Optional<Commande> findById(UUID id) {
        return commandeRepository.findById(id);
    }

    public Page<Commande> findByClientId(UUID clientId, CommandeStatus status, int page, int size) {
        PageRequest pageRequest = PageRequest.of(page, size, Sort.by(Sort.Direction.DESC, "createdAt"));
        if (status != null) {
            return commandeRepository.findByClientIdAndStatus(clientId, status, pageRequest);
        }
        return commandeRepository.findByClientId(clientId, pageRequest);
    }

    public Page<Commande> findByEleveurId(UUID eleveurId, CommandeStatus status, int page, int size) {
        PageRequest pageRequest = PageRequest.of(page, size, Sort.by(Sort.Direction.DESC, "createdAt"));
        if (status != null) {
            return commandeRepository.findByEleveurIdAndStatus(eleveurId, status, pageRequest);
        }
        return commandeRepository.findByEleveurId(eleveurId, pageRequest);
    }

    public Commande create(UUID clientId, UUID eleveurId, List<Map<String, Object>> itemInputs) {
        int maxRetries = 3;
        for (int attempt = 1; attempt <= maxRetries; attempt++) {
            try {
                return doCreate(clientId, eleveurId, itemInputs);
            } catch (OptimisticLockingFailureException ex) {
                if (attempt == maxRetries) {
                    log.warn("Stock contention after {} attempts for clientId={}", maxRetries, clientId);
                    throw new IllegalStateException("Stock épuisé — réessayez dans quelques instants", ex);
                }
                log.debug("Optimistic lock conflict on attempt {}, retrying...", attempt);
            }
        }
        throw new IllegalStateException("Stock épuisé");
    }

    @Transactional
    protected Commande doCreate(UUID clientId, UUID eleveurId, List<Map<String, Object>> itemInputs) {
        Client client = clientRepository.findById(clientId)
                .orElseThrow(() -> new IllegalArgumentException("Client not found: " + clientId));

        Eleveur eleveur = eleveurRepository.findById(eleveurId)
                .orElseThrow(() -> new IllegalArgumentException("Eleveur not found: " + eleveurId));

        Commande commande = new Commande();
        commande.setClient(client);
        commande.setEleveur(eleveur);
        commande.setStatus(CommandeStatus.PENDING);

        double totalAmount = 0.0;

        // Save commande first to get ID for items
        Commande saved = commandeRepository.save(commande);

        for (Map<String, Object> itemInput : itemInputs) {
            UUID pouletId = UUID.fromString((String) itemInput.get("pouletId"));
            int qty = ((Number) itemInput.get("quantity")).intValue();

            Poulet poulet = pouletRepository.findById(pouletId)
                    .orElseThrow(() -> new IllegalArgumentException("Poulet not found: " + pouletId));

            if (!poulet.isAvailable() || poulet.getQuantity() < qty) {
                throw new IllegalArgumentException(
                        "Insufficient stock for poulet " + pouletId +
                        ": requested=" + qty + " available=" + poulet.getQuantity());
            }

            // Decrement stock — @Version on Poulet will throw OptimisticLockingFailureException
            // if another transaction modified this row concurrently
            poulet.setQuantity(poulet.getQuantity() - qty);
            if (poulet.getQuantity() == 0) {
                poulet.setAvailable(false);
            }
            pouletRepository.save(poulet);

            CommandeItem item = new CommandeItem();
            item.setCommande(saved);
            item.setPoulet(poulet);
            item.setQuantity(qty);
            item.setUnitPrice(poulet.getPrice());

            saved.getItems().add(item);
            totalAmount += poulet.getPrice() * qty;
        }

        saved.setTotalAmount(totalAmount);
        saved = commandeRepository.save(saved);

        log.info("Created commande: id={} clientId={} eleveurId={} totalAmount={}",
                saved.getId(), clientId, eleveurId, totalAmount);
        return saved;
    }

    @Transactional
    public Commande cancel(UUID id) {
        Commande commande = commandeRepository.findById(id)
                .orElseThrow(() -> new IllegalArgumentException("Commande not found: " + id));

        if (commande.getStatus() == CommandeStatus.DELIVERED) {
            throw new IllegalStateException("Cannot cancel a delivered order");
        }
        if (commande.getStatus() == CommandeStatus.CANCELLED) {
            throw new IllegalStateException("Order is already cancelled");
        }

        // Restore stock
        for (CommandeItem item : commande.getItems()) {
            Poulet poulet = item.getPoulet();
            poulet.setQuantity(poulet.getQuantity() + item.getQuantity());
            poulet.setAvailable(true);
            pouletRepository.save(poulet);
        }

        commande.setStatus(CommandeStatus.CANCELLED);
        Commande saved = commandeRepository.save(commande);
        log.info("Cancelled commande: id={}", id);
        return saved;
    }

    @Transactional
    public Commande confirm(UUID id) {
        return transitionStatus(id, CommandeStatus.PENDING, CommandeStatus.CONFIRMED);
    }

    @Transactional
    public Commande markReady(UUID id) {
        Commande commande = commandeRepository.findById(id)
                .orElseThrow(() -> new IllegalArgumentException("Commande not found: " + id));

        if (commande.getStatus() != CommandeStatus.CONFIRMED &&
            commande.getStatus() != CommandeStatus.PREPARING) {
            throw new IllegalStateException(
                    "Cannot mark as ready from status: " + commande.getStatus());
        }

        commande.setStatus(CommandeStatus.READY);
        Commande saved = commandeRepository.save(commande);
        log.info("Commande marked ready: id={}", id);
        return saved;
    }

    @Transactional
    public Commande markDelivered(UUID id) {
        Commande commande = commandeRepository.findById(id)
                .orElseThrow(() -> new IllegalArgumentException("Commande not found: " + id));

        if (commande.getStatus() != CommandeStatus.READY) {
            throw new IllegalStateException(
                    "Cannot mark as delivered from status: " + commande.getStatus());
        }

        commande.setStatus(CommandeStatus.DELIVERED);
        commande.setDeliveredAt(Instant.now());
        Commande saved = commandeRepository.save(commande);
        log.info("Commande delivered: id={}", id);
        return saved;
    }

    private Commande transitionStatus(UUID id, CommandeStatus expected, CommandeStatus target) {
        Commande commande = commandeRepository.findById(id)
                .orElseThrow(() -> new IllegalArgumentException("Commande not found: " + id));

        if (commande.getStatus() != expected) {
            throw new IllegalStateException(
                    "Cannot transition from " + commande.getStatus() + " to " + target +
                    ". Expected current status: " + expected);
        }

        commande.setStatus(target);
        Commande saved = commandeRepository.save(commande);
        log.info("Commande status changed: id={} {} -> {}", id, expected, target);
        return saved;
    }
}
