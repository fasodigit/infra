package bf.gov.faso.auth.grpc;

import bf.gov.faso.auth.grpc.proto.*;
import bf.gov.faso.auth.model.Permission;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.*;
import com.google.protobuf.Timestamp;
import com.nimbusds.jwt.JWTClaimsSet;
import io.grpc.Status;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.time.Instant;
import java.util.*;
import java.util.stream.Collectors;

/**
 * gRPC service implementation for Est-Ouest internal communications.
 * <p>
 * Exposes:
 * - ValidateToken: verify a JWT and return claims
 * - GetUserPermissions: retrieve Zanzibar relation tuples for a user
 * - SyncKetoRelations: trigger a full or partial Keto sync
 */
@GrpcService
public class AuthGrpcService extends AuthServiceGrpc.AuthServiceImplBase {

    private static final Logger log = LoggerFactory.getLogger(AuthGrpcService.class);

    private final JwtService jwtService;
    private final JtiBlacklistService blacklistService;
    private final KetoService ketoService;
    private final PermissionGrantService permissionGrantService;
    private final UserRepository userRepository;

    public AuthGrpcService(JwtService jwtService,
                           JtiBlacklistService blacklistService,
                           KetoService ketoService,
                           PermissionGrantService permissionGrantService,
                           UserRepository userRepository) {
        this.jwtService = jwtService;
        this.blacklistService = blacklistService;
        this.ketoService = ketoService;
        this.permissionGrantService = permissionGrantService;
        this.userRepository = userRepository;
    }

    @Override
    public void validateToken(ValidateTokenRequest request,
                              StreamObserver<ValidateTokenResponse> responseObserver) {
        try {
            String token = request.getToken();
            if (token == null || token.isBlank()) {
                responseObserver.onNext(ValidateTokenResponse.newBuilder()
                        .setValid(false)
                        .setErrorMessage("Token is empty")
                        .build());
                responseObserver.onCompleted();
                return;
            }

            Optional<JWTClaimsSet> claimsOpt = jwtService.verifyToken(token);
            if (claimsOpt.isEmpty()) {
                responseObserver.onNext(ValidateTokenResponse.newBuilder()
                        .setValid(false)
                        .setErrorMessage("Invalid or expired token")
                        .build());
                responseObserver.onCompleted();
                return;
            }

            JWTClaimsSet claims = claimsOpt.get();

            // Check blacklist
            String jti = claims.getJWTID();
            if (jti != null && blacklistService.isBlacklisted(jti)) {
                responseObserver.onNext(ValidateTokenResponse.newBuilder()
                        .setValid(false)
                        .setErrorMessage("Token has been revoked")
                        .build());
                responseObserver.onCompleted();
                return;
            }

            // Build successful response
            ValidateTokenResponse.Builder responseBuilder = ValidateTokenResponse.newBuilder()
                    .setValid(true)
                    .setUserId(claims.getSubject());

            String email = claims.getStringClaim("email");
            if (email != null) {
                responseBuilder.setEmail(email);
            }

            @SuppressWarnings("unchecked")
            List<String> roles = (List<String>) claims.getClaim("roles");
            if (roles != null) {
                responseBuilder.addAllRoles(roles);
            }

            if (claims.getExpirationTime() != null) {
                Instant exp = claims.getExpirationTime().toInstant();
                responseBuilder.setExpiresAt(Timestamp.newBuilder()
                        .setSeconds(exp.getEpochSecond())
                        .setNanos(exp.getNano())
                        .build());
            }

            responseObserver.onNext(responseBuilder.build());
            responseObserver.onCompleted();

        } catch (Exception e) {
            log.error("ValidateToken gRPC error: {}", e.getMessage());
            responseObserver.onError(Status.INTERNAL
                    .withDescription("Token validation failed: " + e.getMessage())
                    .asRuntimeException());
        }
    }

    @Override
    public void getUserPermissions(GetUserPermissionsRequest request,
                                   StreamObserver<GetUserPermissionsResponse> responseObserver) {
        try {
            String userId = request.getUserId();
            if (userId == null || userId.isBlank()) {
                responseObserver.onError(Status.INVALID_ARGUMENT
                        .withDescription("user_id is required")
                        .asRuntimeException());
                return;
            }

            UUID uid = UUID.fromString(userId);
            Set<Permission> permissions = permissionGrantService.getEffectivePermissions(uid);

            String namespaceFilter = request.getNamespace();
            List<RelationTuple> tuples = permissions.stream()
                    .filter(p -> namespaceFilter == null || namespaceFilter.isBlank()
                            || p.getNamespace().equals(namespaceFilter))
                    .map(p -> RelationTuple.newBuilder()
                            .setNamespace(p.getNamespace())
                            .setObject(p.getObject())
                            .setRelation(p.getRelation())
                            .setSubjectId(userId)
                            .build())
                    .collect(Collectors.toList());

            responseObserver.onNext(GetUserPermissionsResponse.newBuilder()
                    .setUserId(userId)
                    .addAllPermissions(tuples)
                    .build());
            responseObserver.onCompleted();

        } catch (IllegalArgumentException e) {
            responseObserver.onError(Status.NOT_FOUND
                    .withDescription(e.getMessage())
                    .asRuntimeException());
        } catch (Exception e) {
            log.error("GetUserPermissions gRPC error: {}", e.getMessage());
            responseObserver.onError(Status.INTERNAL
                    .withDescription("Failed to get permissions: " + e.getMessage())
                    .asRuntimeException());
        }
    }

    @Override
    public void syncKetoRelations(SyncKetoRelationsRequest request,
                                  StreamObserver<SyncKetoRelationsResponse> responseObserver) {
        try {
            int synced;
            if (request.getFullSync()) {
                log.info("Starting full Keto sync via gRPC");
                synced = ketoService.fullSync();
            } else if (!request.getUserIdsList().isEmpty()) {
                log.info("Starting partial Keto sync for {} users via gRPC",
                        request.getUserIdsList().size());
                synced = ketoService.syncUsers(request.getUserIdsList());
            } else {
                // Default to full sync if no user IDs specified
                synced = ketoService.fullSync();
            }

            Instant now = Instant.now();
            responseObserver.onNext(SyncKetoRelationsResponse.newBuilder()
                    .setSuccess(true)
                    .setTuplesSynced(synced)
                    .setSyncedAt(Timestamp.newBuilder()
                            .setSeconds(now.getEpochSecond())
                            .setNanos(now.getNano())
                            .build())
                    .build());
            responseObserver.onCompleted();

        } catch (Exception e) {
            log.error("SyncKetoRelations gRPC error: {}", e.getMessage());
            responseObserver.onNext(SyncKetoRelationsResponse.newBuilder()
                    .setSuccess(false)
                    .setErrorMessage("Sync failed: " + e.getMessage())
                    .build());
            responseObserver.onCompleted();
        }
    }
}
