// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.servlet.http.HttpServletRequest;
import org.aspectj.lang.ProceedingJoinPoint;
import org.aspectj.lang.annotation.Around;
import org.aspectj.lang.annotation.Aspect;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Component;
import org.springframework.web.context.request.RequestContextHolder;
import org.springframework.web.context.request.ServletRequestAttributes;

import java.util.Map;

/**
 * AOP aspect that intercepts methods annotated with {@link Audited} and
 * records an {@link AuditEvent} via {@link AuditService}.
 *
 * <p>Extracts actor identity from Spring Security context and client IP /
 * User-Agent from the current HTTP request (if available).
 */
@Aspect
@Component
public class AuditAspect {

    private static final ObjectMapper JSON = new ObjectMapper();

    private final AuditService auditService;

    public AuditAspect(AuditService auditService) {
        this.auditService = auditService;
    }

    @Around("@annotation(audited)")
    public Object audit(ProceedingJoinPoint joinPoint, Audited audited) throws Throwable {
        Authentication auth = SecurityContextHolder.getContext().getAuthentication();
        String actorId = auth != null ? auth.getName() : null;
        String ip = null;
        String userAgent = null;

        ServletRequestAttributes attrs =
                (ServletRequestAttributes) RequestContextHolder.getRequestAttributes();
        if (attrs != null) {
            HttpServletRequest request = attrs.getRequest();
            ip = request.getHeader("X-Forwarded-For");
            if (ip == null) {
                ip = request.getRemoteAddr();
            }
            userAgent = request.getHeader("User-Agent");
        }

        try {
            Object returnValue = joinPoint.proceed();
            auditService.record(AuditEvent.builder()
                    .actorId(actorId)
                    .actorType(auth != null ? AuditEvent.ActorType.USER : AuditEvent.ActorType.ANONYMOUS)
                    .action(audited.action())
                    .resourceType(audited.resourceType())
                    .ipAddress(ip)
                    .userAgent(userAgent)
                    .result(AuditEvent.AuditResult.SUCCESS)
                    .build());
            return returnValue;
        } catch (Throwable e) {
            auditService.record(AuditEvent.builder()
                    .actorId(actorId)
                    .actorType(auth != null ? AuditEvent.ActorType.USER : AuditEvent.ActorType.ANONYMOUS)
                    .action(audited.action())
                    .resourceType(audited.resourceType())
                    .ipAddress(ip)
                    .userAgent(userAgent)
                    .result(AuditEvent.AuditResult.FAILURE)
                    .metadata(serializeError(e))
                    .build());
            throw e;
        }
    }

    /**
     * Build a safe JSON metadata payload describing the failure.
     *
     * <p>Handles null exception messages (common with NPE / custom RuntimeException
     * subclasses) and any control characters in the message via Jackson encoding.
     */
    private static String serializeError(Throwable e) {
        String message = e.getMessage();
        String fallback = e.getClass().getSimpleName();
        try {
            return JSON.writeValueAsString(Map.of(
                    "error", message != null ? message : fallback,
                    "type", e.getClass().getName()));
        } catch (JsonProcessingException ex) {
            return "{\"error\":\"" + fallback + "\",\"type\":\"" + e.getClass().getName() + "\"}";
        }
    }
}
