package bf.gov.faso.auth.config;

import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.client.reactive.ReactorClientHttpConnector;
import org.springframework.web.reactive.function.client.WebClient;
import reactor.netty.http.client.HttpClient;

import java.time.Duration;

/**
 * Configuration for Ory Kratos HTTP clients.
 * Provides separate WebClient beans for public and admin APIs.
 */
@Configuration
public class KratosConfig {

    @Value("${kratos.public-url}")
    private String kratosPublicUrl;

    @Value("${kratos.admin-url}")
    private String kratosAdminUrl;

    @Value("${kratos.timeout-ms:5000}")
    private int timeoutMs;

    @Bean(name = "kratosPublicClient")
    public WebClient kratosPublicClient(WebClient.Builder builder) {
        return buildClient(builder, kratosPublicUrl);
    }

    @Bean(name = "kratosAdminClient")
    public WebClient kratosAdminClient(WebClient.Builder builder) {
        return buildClient(builder, kratosAdminUrl);
    }

    private WebClient buildClient(WebClient.Builder builder, String baseUrl) {
        HttpClient httpClient = HttpClient.create()
                .responseTimeout(Duration.ofMillis(timeoutMs));

        return builder
                .baseUrl(baseUrl)
                .clientConnector(new ReactorClientHttpConnector(httpClient))
                .defaultHeader("Accept", "application/json")
                .defaultHeader("Content-Type", "application/json")
                .build();
    }
}
