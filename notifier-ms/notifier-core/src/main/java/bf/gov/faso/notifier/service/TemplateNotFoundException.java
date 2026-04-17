/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.service;

/**
 * TemplateNotFoundException — thrown when no Handlebars template is found for the given name.
 * This exception is marked as non-retryable in Resilience4j configuration.
 */
public class TemplateNotFoundException extends RuntimeException {

    public TemplateNotFoundException(String templateName) {
        super("No template found with name: " + templateName);
    }
}
