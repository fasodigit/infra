package bf.gov.faso.poulets.grpc;

import bf.gov.faso.poulets.grpc.proto.*;
import bf.gov.faso.poulets.model.Commande;
import bf.gov.faso.poulets.model.Eleveur;
import bf.gov.faso.poulets.model.Poulet;
import bf.gov.faso.poulets.repository.CommandeRepository;
import bf.gov.faso.poulets.repository.EleveurRepository;
import bf.gov.faso.poulets.repository.PouletRepository;
import com.google.protobuf.Timestamp;
import io.grpc.Status;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

/**
 * gRPC service implementation for Est-Ouest internal communications.
 * <p>
 * Exposes:
 * - GetPoulet: retrieve poulet details
 * - CheckStock: verify stock availability
 * - GetEleveur: retrieve eleveur details
 * - GetCommande: retrieve commande details
 */
@GrpcService
public class PouletsGrpcService extends PouletsServiceGrpc.PouletsServiceImplBase {

    private static final Logger log = LoggerFactory.getLogger(PouletsGrpcService.class);

    private final PouletRepository pouletRepository;
    private final EleveurRepository eleveurRepository;
    private final CommandeRepository commandeRepository;

    public PouletsGrpcService(PouletRepository pouletRepository,
                              EleveurRepository eleveurRepository,
                              CommandeRepository commandeRepository) {
        this.pouletRepository = pouletRepository;
        this.eleveurRepository = eleveurRepository;
        this.commandeRepository = commandeRepository;
    }

    @Override
    public void getPoulet(GetPouletRequest request,
                          StreamObserver<GetPouletResponse> responseObserver) {
        try {
            UUID id = UUID.fromString(request.getPouletId());
            Optional<Poulet> pouletOpt = pouletRepository.findById(id);

            if (pouletOpt.isEmpty()) {
                responseObserver.onError(Status.NOT_FOUND
                        .withDescription("Poulet not found: " + request.getPouletId())
                        .asRuntimeException());
                return;
            }

            Poulet poulet = pouletOpt.get();
            GetPouletResponse.Builder builder = GetPouletResponse.newBuilder()
                    .setId(poulet.getId().toString())
                    .setRace(poulet.getRace().name())
                    .setWeight(poulet.getWeight())
                    .setPrice(poulet.getPrice())
                    .setQuantity(poulet.getQuantity())
                    .setAvailable(poulet.isAvailable());

            if (poulet.getEleveurId() != null) {
                builder.setEleveurId(poulet.getEleveurId().toString());
            }
            if (poulet.getDescription() != null) {
                builder.setDescription(poulet.getDescription());
            }

            responseObserver.onNext(builder.build());
            responseObserver.onCompleted();
        } catch (Exception e) {
            log.error("GetPoulet gRPC error: {}", e.getMessage());
            responseObserver.onError(Status.INTERNAL
                    .withDescription("Failed to get poulet: " + e.getMessage())
                    .asRuntimeException());
        }
    }

    @Override
    public void checkStock(CheckStockRequest request,
                           StreamObserver<CheckStockResponse> responseObserver) {
        try {
            boolean allAvailable = true;
            List<StockItemStatus> statuses = new ArrayList<>();

            for (StockItem item : request.getItemsList()) {
                UUID pouletId = UUID.fromString(item.getPouletId());
                Optional<Poulet> pouletOpt = pouletRepository.findById(pouletId);

                StockItemStatus.Builder statusBuilder = StockItemStatus.newBuilder()
                        .setPouletId(item.getPouletId())
                        .setRequestedQuantity(item.getRequestedQuantity());

                if (pouletOpt.isEmpty()) {
                    statusBuilder.setAvailable(false).setCurrentQuantity(0);
                    allAvailable = false;
                } else {
                    Poulet poulet = pouletOpt.get();
                    boolean available = poulet.isAvailable() &&
                            poulet.getQuantity() >= item.getRequestedQuantity();
                    statusBuilder.setAvailable(available)
                            .setCurrentQuantity(poulet.getQuantity());
                    if (!available) allAvailable = false;
                }

                statuses.add(statusBuilder.build());
            }

            responseObserver.onNext(CheckStockResponse.newBuilder()
                    .setAllAvailable(allAvailable)
                    .addAllStatuses(statuses)
                    .build());
            responseObserver.onCompleted();
        } catch (Exception e) {
            log.error("CheckStock gRPC error: {}", e.getMessage());
            responseObserver.onError(Status.INTERNAL
                    .withDescription("Failed to check stock: " + e.getMessage())
                    .asRuntimeException());
        }
    }

    @Override
    public void getEleveur(GetEleveurRequest request,
                           StreamObserver<GetEleveurResponse> responseObserver) {
        try {
            UUID id = UUID.fromString(request.getEleveurId());
            Optional<Eleveur> eleveurOpt = eleveurRepository.findById(id);

            if (eleveurOpt.isEmpty()) {
                responseObserver.onError(Status.NOT_FOUND
                        .withDescription("Eleveur not found: " + request.getEleveurId())
                        .asRuntimeException());
                return;
            }

            Eleveur eleveur = eleveurOpt.get();
            responseObserver.onNext(GetEleveurResponse.newBuilder()
                    .setId(eleveur.getId().toString())
                    .setUserId(eleveur.getUserId())
                    .setName(eleveur.getName())
                    .setPhone(eleveur.getPhone())
                    .setLocation(eleveur.getLocation())
                    .setRating(eleveur.getRating())
                    .setActive(eleveur.isActive())
                    .build());
            responseObserver.onCompleted();
        } catch (Exception e) {
            log.error("GetEleveur gRPC error: {}", e.getMessage());
            responseObserver.onError(Status.INTERNAL
                    .withDescription("Failed to get eleveur: " + e.getMessage())
                    .asRuntimeException());
        }
    }

    @Override
    public void getCommande(GetCommandeRequest request,
                            StreamObserver<GetCommandeResponse> responseObserver) {
        try {
            UUID id = UUID.fromString(request.getCommandeId());
            Optional<Commande> commandeOpt = commandeRepository.findById(id);

            if (commandeOpt.isEmpty()) {
                responseObserver.onError(Status.NOT_FOUND
                        .withDescription("Commande not found: " + request.getCommandeId())
                        .asRuntimeException());
                return;
            }

            Commande commande = commandeOpt.get();
            GetCommandeResponse.Builder builder = GetCommandeResponse.newBuilder()
                    .setId(commande.getId().toString())
                    .setStatus(commande.getStatus().name())
                    .setTotalAmount(commande.getTotalAmount());

            if (commande.getClientId() != null) {
                builder.setClientId(commande.getClientId().toString());
            }
            if (commande.getEleveurId() != null) {
                builder.setEleveurId(commande.getEleveurId().toString());
            }

            Instant createdAt = commande.getCreatedAt();
            if (createdAt != null) {
                builder.setCreatedAt(Timestamp.newBuilder()
                        .setSeconds(createdAt.getEpochSecond())
                        .setNanos(createdAt.getNano())
                        .build());
            }

            if (commande.getDeliveredAt() != null) {
                Instant deliveredAt = commande.getDeliveredAt();
                builder.setDeliveredAt(Timestamp.newBuilder()
                        .setSeconds(deliveredAt.getEpochSecond())
                        .setNanos(deliveredAt.getNano())
                        .build());
            }

            responseObserver.onNext(builder.build());
            responseObserver.onCompleted();
        } catch (Exception e) {
            log.error("GetCommande gRPC error: {}", e.getMessage());
            responseObserver.onError(Status.INTERNAL
                    .withDescription("Failed to get commande: " + e.getMessage())
                    .asRuntimeException());
        }
    }
}
