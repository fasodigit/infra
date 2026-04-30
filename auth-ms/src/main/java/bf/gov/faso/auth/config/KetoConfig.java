package bf.gov.faso.auth.config;

import io.netty.channel.ChannelOption;
import io.netty.handler.timeout.ReadTimeoutHandler;
import io.netty.handler.timeout.WriteTimeoutHandler;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.client.reactive.ReactorClientHttpConnector;
import org.springframework.web.reactive.function.client.WebClient;
import reactor.netty.http.client.HttpClient;

import java.time.Duration;
import java.util.concurrent.TimeUnit;

/**
 * Configuration for Ory Keto HTTP clients.
 * Provides separate WebClient beans for read and write APIs
 * with connection, read, and write timeouts.
 */
@Configuration
public class KetoConfig {

    @Value("${keto.read-url}")
    private String ketoReadUrl;

    @Value("${keto.write-url}")
    private String ketoWriteUrl;

    @Value("${keto.timeout-ms:5000}")
    private int timeoutMs;

    @Bean(name = "ketoReadClient")
    public WebClient ketoReadClient(WebClient.Builder builder) {
        return buildClient(builder, ketoReadUrl);
    }

    @Bean(name = "ketoWriteClient")
    public WebClient ketoWriteClient(WebClient.Builder builder) {
        return buildClient(builder, ketoWriteUrl);
    }

    private WebClient buildClient(WebClient.Builder builder, String baseUrl) {
        HttpClient httpClient = HttpClient.create()
                .option(ChannelOption.CONNECT_TIMEOUT_MILLIS, 2000)
                .responseTimeout(Duration.ofMillis(timeoutMs))
                .doOnConnected(conn -> conn
                        .addHandlerLast(new ReadTimeoutHandler(5, TimeUnit.SECONDS))
                        .addHandlerLast(new WriteTimeoutHandler(5, TimeUnit.SECONDS)));

        return builder
                .baseUrl(baseUrl)
                .clientConnector(new ReactorClientHttpConnector(httpClient))
                .defaultHeader("Accept", "application/json")
                .defaultHeader("Content-Type", "application/json")
                .build();
    }
}
