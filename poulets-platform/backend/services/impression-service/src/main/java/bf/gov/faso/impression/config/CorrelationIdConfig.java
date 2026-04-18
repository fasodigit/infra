package bf.gov.faso.impression.config;

import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.MDC;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.servlet.HandlerInterceptor;
import org.springframework.web.servlet.config.annotation.InterceptorRegistry;
import org.springframework.web.servlet.config.annotation.WebMvcConfigurer;

import java.util.UUID;

@Configuration
public class CorrelationIdConfig implements WebMvcConfigurer {

    @Override
    public void addInterceptors(InterceptorRegistry registry) {
        registry.addInterceptor(new CorrelationInterceptor()).addPathPatterns("/**");
    }

    static class CorrelationInterceptor implements HandlerInterceptor {

        @Override
        public boolean preHandle(HttpServletRequest request, HttpServletResponse response, Object handler) {
            String correlationId = request.getHeader("X-Correlation-ID");
            if (correlationId == null || correlationId.isBlank()) {
                correlationId = UUID.randomUUID().toString();
            }
            MDC.put("correlationId", correlationId);

            String tenantId = request.getHeader("X-Tenant-ID");
            if (tenantId != null && !tenantId.isBlank()) {
                MDC.put("tenantId", tenantId);
            }

            response.setHeader("X-Correlation-ID", correlationId);
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
