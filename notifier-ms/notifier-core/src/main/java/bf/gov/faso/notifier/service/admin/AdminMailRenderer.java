/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.service.admin;

import bf.gov.faso.notifier.service.HandlebarsHelpers;
import bf.gov.faso.notifier.service.TemplateNotFoundException;
import com.github.jknack.handlebars.Handlebars;
import com.github.jknack.handlebars.Template;
import com.github.jknack.handlebars.io.ClassPathTemplateLoader;
import com.github.jknack.handlebars.io.TemplateLoader;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.io.IOException;
import java.util.HashMap;
import java.util.Map;

/**
 * AdminMailRenderer — Handlebars renderer dedicated to the admin-* templates
 * located in {@code /templates/admin/}.
 *
 * <p>Returns subject, HTML body and plain-text body for a given template name and
 * context map. Bilingual support (FR default, EN if {@code lang=en} is present in
 * the variables map).
 */
@Service
public class AdminMailRenderer {

    private static final Logger log = LoggerFactory.getLogger(AdminMailRenderer.class);

    private final Handlebars handlebars;

    public AdminMailRenderer() {
        TemplateLoader loader = new ClassPathTemplateLoader("/templates", ".hbs");
        this.handlebars = new Handlebars(loader);
        this.handlebars.registerHelpers(HandlebarsHelpers.class);
    }

    /**
     * Render an admin template (e.g. {@code "admin/otp-email"}) against the given variables.
     *
     * @param templateName classpath-relative name without extension (e.g. {@code "admin/otp-email"})
     * @param vars         variables exposed to the template; may contain {@code "lang"} ("fr"|"en")
     * @return subject + HTML body + plain-text body
     */
    public RenderedAdminMail render(String templateName, Map<String, Object> vars) {
        Map<String, Object> ctx = new HashMap<>(vars != null ? vars : Map.of());
        ctx.putIfAbsent("lang", "fr");
        String lang = String.valueOf(ctx.get("lang"));

        try {
            Template bodyTpl = handlebars.compile(templateName);
            String html = bodyTpl.apply(ctx);
            String subject = subjectFor(templateName, ctx, lang);
            String text = htmlToPlainText(html);
            return new RenderedAdminMail(subject, html, text);
        } catch (IOException e) {
            log.error("Admin template not found or failed to compile: {}", templateName);
            throw new TemplateNotFoundException(templateName);
        }
    }

    private String subjectFor(String templateName, Map<String, Object> ctx, String lang) {
        boolean en = "en".equalsIgnoreCase(lang);
        return switch (templateName) {
            case "admin/otp-email" -> en
                ? "[FASO ADMIN] Your one-time access code"
                : "[FASO ADMIN] Votre code d'accès à usage unique";
            case "admin/admin-invitation" -> en
                ? "[FASO ADMIN] You have been invited as " + ctx.getOrDefault("role", "admin")
                : "[FASO ADMIN] Invitation au rôle " + ctx.getOrDefault("role", "administrateur");
            case "admin/admin-role-granted" -> en
                ? "[FASO ADMIN] Role granted: " + ctx.getOrDefault("targetRole", "")
                : "[FASO ADMIN] Rôle accordé : " + ctx.getOrDefault("targetRole", "");
            case "admin/admin-role-grant-approval-required" -> en
                ? "[FASO ADMIN] Approval required: role grant for " + ctx.getOrDefault("target", "")
                : "[FASO ADMIN] Approbation requise : attribution de rôle pour " + ctx.getOrDefault("target", "");
            case "admin/admin-mfa-enrollment-instruction" -> en
                ? "[FASO ADMIN] MFA enrollment instructions"
                : "[FASO ADMIN] Instructions d'enrôlement MFA";
            case "admin/admin-recovery-codes" -> en
                ? "[FASO ADMIN] Your recovery codes (single-use)"
                : "[FASO ADMIN] Vos codes de récupération (à usage unique)";
            case "admin/admin-break-glass-activated" -> en
                ? "[FASO ADMIN][HIGH] BREAK-GLASS activated by " + ctx.getOrDefault("activator", "")
                : "[FASO ADMIN][HAUTE] BREAK-GLASS activé par " + ctx.getOrDefault("activator", "");
            case "admin/admin-session-revoked" -> en
                ? "[FASO ADMIN] One of your sessions has been revoked"
                : "[FASO ADMIN] L'une de vos sessions a été révoquée";
            case "admin/admin-settings-changed" -> en
                ? "[FASO ADMIN] Admin settings have changed (digest)"
                : "[FASO ADMIN] Paramètres administrateur modifiés (digest)";
            case "admin/admin-recovery-self-link" -> en
                ? "FASO DIGITALISATION — Account recovery"
                : "FASO DIGITALISATION — Récupération de votre compte";
            case "admin/admin-recovery-admin-token" -> en
                ? "FASO DIGITALISATION — Administrator recovery code"
                : "FASO DIGITALISATION — Code de récupération administrateur";
            case "admin/admin-recovery-completed" -> en
                ? "FASO DIGITALISATION — Recovery completed · MFA reset"
                : "FASO DIGITALISATION — Récupération terminée · MFA réinitialisée";
            default -> "[FASO ADMIN] " + templateName;
        };
    }

    private String htmlToPlainText(String html) {
        if (html == null) return "";
        // Best-effort fallback: strip tags and collapse whitespace
        return html
            .replaceAll("(?is)<style.*?</style>", "")
            .replaceAll("(?is)<script.*?</script>", "")
            .replaceAll("(?is)<br\\s*/?>", "\n")
            .replaceAll("(?is)</p>", "\n\n")
            .replaceAll("<[^>]+>", "")
            .replaceAll("[ \\t]+", " ")
            .replaceAll("\\n{3,}", "\n\n")
            .trim();
    }

    /** Result of an admin template rendering. */
    public record RenderedAdminMail(String subject, String html, String text) {}
}
