package bf.gov.actes.security.config;

import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.MDC;
import org.springframework.boot.autoconfigure.AutoConfiguration;
import org.springframework.boot.autoconfigure.condition.ConditionalOnWebApplication;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.servlet.HandlerInterceptor;
import org.springframework.web.servlet.config.annotation.InterceptorRegistry;
import org.springframework.web.servlet.config.annotation.WebMvcConfigurer;

import java.util.UUID;

/**
 * Auto-configures correlation ID propagation via Spring MVC interceptor.
 * Sets X-Correlation-ID in response headers and populates SLF4J MDC.
 */
@AutoConfiguration
@ConditionalOnWebApplication(type = ConditionalOnWebApplication.Type.SERVLET)
public class CorrelationIdAutoConfiguration implements WebMvcConfigurer {

    public static final String CORRELATION_ID_HEADER = "X-Correlation-ID";
    public static final String TENANT_ID_HEADER = "X-Tenant-ID";

    @Override
    public void addInterceptors(InterceptorRegistry registry) {
        registry.addInterceptor(new CorrelationIdInterceptor()).addPathPatterns("/**");
    }

    static class CorrelationIdInterceptor implements HandlerInterceptor {

        @Override
        public boolean preHandle(HttpServletRequest request, HttpServletResponse response, Object handler) {
            String correlationId = request.getHeader(CORRELATION_ID_HEADER);
            if (correlationId == null || correlationId.isBlank()) {
                correlationId = UUID.randomUUID().toString();
            }
            MDC.put("correlationId", correlationId);

            String tenantId = request.getHeader(TENANT_ID_HEADER);
            if (tenantId != null && !tenantId.isBlank()) {
                MDC.put("tenantId", tenantId);
            }

            response.setHeader(CORRELATION_ID_HEADER, correlationId);
            return true;
        }

        @Override
        public void afterCompletion(HttpServletRequest request, HttpServletResponse response,
                                     Object handler, Exception ex) {
            MDC.remove("correlationId");
            MDC.remove("tenantId");
        }
    }
}
