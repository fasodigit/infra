package bf.gov.actes.security.filter;

import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.slf4j.MDC;
import org.springframework.core.Ordered;
import org.springframework.core.annotation.Order;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

import java.io.IOException;
import java.util.UUID;

/**
 * Servlet filter that propagates correlation and tenant IDs through the request lifecycle.
 * <p>
 * Reads {@code X-Correlation-ID} from incoming HTTP headers (or generates a new UUID if absent),
 * places it in the SLF4J MDC for structured logging, and echoes it back in the response headers.
 * Also propagates {@code X-Tenant-ID} into MDC when present.
 * <p>
 * Runs at {@link Ordered#HIGHEST_PRECEDENCE} so that all downstream filters and handlers
 * have access to the correlation context.
 *
 * @since 1.0.0
 */
@Component
@Order(Ordered.HIGHEST_PRECEDENCE)
public class CorrelationIdFilter extends OncePerRequestFilter {

    private static final Logger log = LoggerFactory.getLogger(CorrelationIdFilter.class);
    public static final String CORRELATION_ID_HEADER = "X-Correlation-ID";
    public static final String TENANT_ID_HEADER = "X-Tenant-ID";
    public static final String MDC_CORRELATION_ID = "correlationId";
    public static final String MDC_TENANT_ID = "tenantId";

    @Override
    protected void doFilterInternal(HttpServletRequest request, HttpServletResponse response,
                                     FilterChain filterChain) throws ServletException, IOException {
        try {
            String correlationId = request.getHeader(CORRELATION_ID_HEADER);
            if (correlationId == null || correlationId.isBlank()) {
                correlationId = UUID.randomUUID().toString();
            }
            MDC.put(MDC_CORRELATION_ID, correlationId);

            String tenantId = request.getHeader(TENANT_ID_HEADER);
            if (tenantId != null && !tenantId.isBlank()) {
                MDC.put(MDC_TENANT_ID, tenantId);
            }

            response.addHeader(CORRELATION_ID_HEADER, correlationId);
            log.trace("Correlation ID: {} for {} {}", correlationId, request.getMethod(), request.getRequestURI());
            filterChain.doFilter(request, response);
        } finally {
            MDC.remove(MDC_CORRELATION_ID);
            MDC.remove(MDC_TENANT_ID);
        }
    }
}
