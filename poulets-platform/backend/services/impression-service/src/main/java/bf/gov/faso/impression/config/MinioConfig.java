package bf.gov.faso.impression.config;

import io.minio.MinioClient;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/**
 * MinIO client configuration for WORM storage.
 */
@Configuration
public class MinioConfig {

    @Value("${minio.endpoint:http://localhost:19100}")
    private String endpoint;

    @Value("${minio.access-key:minioadmin}")
    private String accessKey;

    @Value("${minio.secret-key:minioadmin}")
    private String secretKey;

    @Value("${minio.enabled:true}")
    private boolean enabled;

    @Bean
    public MinioClient minioClient() {
        if (!enabled) {
            // Return a dummy client when disabled
            return MinioClient.builder()
                .endpoint("http://localhost:19100")
                .credentials("dummy", "dummy")
                .build();
        }

        return MinioClient.builder()
            .endpoint(endpoint)
            .credentials(accessKey, secretKey)
            .build();
    }
}
