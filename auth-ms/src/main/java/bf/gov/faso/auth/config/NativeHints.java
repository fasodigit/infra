package bf.gov.faso.auth.config;

import org.springframework.aot.hint.MemberCategory;
import org.springframework.aot.hint.RuntimeHints;
import org.springframework.aot.hint.RuntimeHintsRegistrar;

/**
 * Registers GraalVM Native Image runtime hints for classes and resources
 * that require reflection, resource access, or proxy generation at runtime.
 * <p>
 * Applied via {@code @ImportRuntimeHints} on the main application class.
 */
public class NativeHints implements RuntimeHintsRegistrar {

    @Override
    public void registerHints(RuntimeHints hints, ClassLoader classLoader) {
        // BouncyCastle security provider (reflection-heavy)
        registerReflection(hints, "org.bouncycastle.jce.provider.BouncyCastleProvider");
        registerReflection(hints, "org.bouncycastle.jcajce.provider.asymmetric.ec.KeyFactorySpi$EC");
        registerReflection(hints, "org.bouncycastle.jcajce.provider.asymmetric.ec.KeyPairGeneratorSpi$EC");
        registerReflection(hints, "org.bouncycastle.jcajce.provider.asymmetric.ec.SignatureSpi$ecDSA384");
        registerReflection(hints, "org.bouncycastle.jcajce.provider.asymmetric.EC");

        // GraphQL schema resource
        hints.resources().registerPattern("schema/schema.graphqls");
        hints.resources().registerPattern("db/migration/*.sql");

        // gRPC generated proto classes
        registerReflection(hints, "bf.gov.faso.auth.grpc.proto.AuthProto");
        registerReflection(hints, "bf.gov.faso.auth.grpc.proto.AuthServiceGrpc");
        registerReflection(hints, "bf.gov.faso.auth.grpc.proto.ValidateTokenRequest");
        registerReflection(hints, "bf.gov.faso.auth.grpc.proto.ValidateTokenResponse");
        registerReflection(hints, "bf.gov.faso.auth.grpc.proto.GetUserPermissionsRequest");
        registerReflection(hints, "bf.gov.faso.auth.grpc.proto.GetUserPermissionsResponse");
        registerReflection(hints, "bf.gov.faso.auth.grpc.proto.RelationTuple");
        registerReflection(hints, "bf.gov.faso.auth.grpc.proto.SyncKetoRelationsRequest");
        registerReflection(hints, "bf.gov.faso.auth.grpc.proto.SyncKetoRelationsResponse");
    }

    private void registerReflection(RuntimeHints hints, String className) {
        try {
            hints.reflection().registerType(
                Class.forName(className),
                MemberCategory.DECLARED_FIELDS,
                MemberCategory.INVOKE_DECLARED_CONSTRUCTORS,
                MemberCategory.INVOKE_DECLARED_METHODS,
                MemberCategory.INVOKE_PUBLIC_METHODS
            );
        } catch (ClassNotFoundException e) {
            // Class not on classpath — skip gracefully (e.g., proto classes not yet generated)
        }
    }
}
