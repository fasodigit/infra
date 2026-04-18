package bf.gov.faso.renderer.filter;

import bf.gov.faso.renderer.config.RendererProperties;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.core.annotation.Order;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Component;
import org.springframework.web.server.ServerWebExchange;
import org.springframework.web.server.WebFilter;
import org.springframework.web.server.WebFilterChain;
import reactor.core.publisher.Mono;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.HexFormat;

/**
 * WebFlux filter validating HMAC-SHA256 authentication on all endpoints
 * except /health and /actuator/**.
 *
 * <p>Expected header format: {@code X-Internal-Auth: {timestamp}:{hmac_hex}}
 * where hmac = HMAC-SHA256(secret, timestamp).
 */
@Component
@Order(1)
public class HmacAuthFilter implements WebFilter {

    private static final Logger log = LoggerFactory.getLogger(HmacAuthFilter.class);
    private static final String HMAC_ALGORITHM = "HmacSHA256";
    private static final String AUTH_HEADER = "X-Internal-Auth";

    private final byte[] secretBytes;
    private final long maxDriftMs;

    public HmacAuthFilter(RendererProperties props) {
        this.secretBytes = props.hmacSecret().getBytes(StandardCharsets.UTF_8);
        this.maxDriftMs = props.hmacTimestampDriftMs();
    }

    @Override
    public Mono<Void> filter(ServerWebExchange exchange, WebFilterChain chain) {
        String path = exchange.getRequest().getPath().value();

        // Bypass auth for health and actuator endpoints
        if (path.equals("/health") || path.startsWith("/actuator")) {
            return chain.filter(exchange);
        }

        String authHeader = exchange.getRequest().getHeaders().getFirst(AUTH_HEADER);
        if (authHeader == null || authHeader.isBlank()) {
            log.warn("Missing {} header for {}", AUTH_HEADER, path);
            exchange.getResponse().setStatusCode(HttpStatus.UNAUTHORIZED);
            return exchange.getResponse().setComplete();
        }

        String[] parts = authHeader.split(":", 2);
        if (parts.length != 2) {
            log.warn("Malformed {} header for {}", AUTH_HEADER, path);
            exchange.getResponse().setStatusCode(HttpStatus.UNAUTHORIZED);
            return exchange.getResponse().setComplete();
        }

        String timestamp = parts[0];
        String receivedHmac = parts[1];

        // Validate timestamp drift
        try {
            long ts = Long.parseLong(timestamp);
            long now = System.currentTimeMillis();
            if (Math.abs(now - ts) > maxDriftMs) {
                log.warn("Timestamp drift too large: {}ms (max={}ms) for {}",
                        Math.abs(now - ts), maxDriftMs, path);
                exchange.getResponse().setStatusCode(HttpStatus.UNAUTHORIZED);
                return exchange.getResponse().setComplete();
            }
        } catch (NumberFormatException e) {
            log.warn("Invalid timestamp in {} header for {}", AUTH_HEADER, path);
            exchange.getResponse().setStatusCode(HttpStatus.UNAUTHORIZED);
            return exchange.getResponse().setComplete();
        }

        // Validate HMAC
        try {
            Mac mac = Mac.getInstance(HMAC_ALGORITHM);
            mac.init(new SecretKeySpec(secretBytes, HMAC_ALGORITHM));
            byte[] expectedBytes = mac.doFinal(timestamp.getBytes(StandardCharsets.UTF_8));
            String expectedHmac = HexFormat.of().formatHex(expectedBytes);

            if (!MessageDigest.isEqual(
                    expectedHmac.getBytes(StandardCharsets.UTF_8),
                    receivedHmac.getBytes(StandardCharsets.UTF_8))) {
                log.warn("HMAC mismatch for {}", path);
                exchange.getResponse().setStatusCode(HttpStatus.UNAUTHORIZED);
                return exchange.getResponse().setComplete();
            }
        } catch (Exception e) {
            log.error("HMAC validation error for {}: {}", path, e.getMessage());
            exchange.getResponse().setStatusCode(HttpStatus.INTERNAL_SERVER_ERROR);
            return exchange.getResponse().setComplete();
        }

        return chain.filter(exchange);
    }
}
