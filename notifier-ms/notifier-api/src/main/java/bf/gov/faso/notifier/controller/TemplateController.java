/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.controller;

import bf.gov.faso.notifier.domain.NotificationTemplate;
import bf.gov.faso.notifier.service.TemplateRepository;
import jakarta.validation.Valid;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

/**
 * TemplateController — CRUD REST API for notification templates.
 *
 * <p>Endpoint: {@code /api/templates}
 * Requires JWT with scope {@code notifier:admin}.
 */
@RestController
@RequestMapping("/api/templates")
public class TemplateController {

    private final TemplateRepository templateRepository;

    public TemplateController(TemplateRepository templateRepository) {
        this.templateRepository = templateRepository;
    }

    @GetMapping
    @PreAuthorize("hasAuthority('SCOPE_notifier:read')")
    public Page<NotificationTemplate> list(Pageable pageable) {
        return templateRepository.findAll(pageable);
    }

    @GetMapping("/{name}")
    @PreAuthorize("hasAuthority('SCOPE_notifier:read')")
    public NotificationTemplate getByName(@PathVariable String name) {
        return templateRepository.findByName(name)
            .orElseThrow(() -> new ResponseStatusException(HttpStatus.NOT_FOUND,
                "Template not found: " + name));
    }

    @PostMapping
    @PreAuthorize("hasAuthority('SCOPE_notifier:admin')")
    @ResponseStatus(HttpStatus.CREATED)
    public NotificationTemplate create(@Valid @RequestBody NotificationTemplate template) {
        if (templateRepository.existsByName(template.getName())) {
            throw new ResponseStatusException(HttpStatus.CONFLICT,
                "Template already exists: " + template.getName());
        }
        return templateRepository.save(template);
    }

    @PutMapping("/{name}")
    @PreAuthorize("hasAuthority('SCOPE_notifier:admin')")
    public NotificationTemplate update(
            @PathVariable String name,
            @Valid @RequestBody NotificationTemplate updates) {
        NotificationTemplate existing = templateRepository.findByName(name)
            .orElseThrow(() -> new ResponseStatusException(HttpStatus.NOT_FOUND,
                "Template not found: " + name));
        existing.setSubjectTemplate(updates.getSubjectTemplate());
        existing.setBodyHbs(updates.getBodyHbs());
        existing.setContextRulesJson(updates.getContextRulesJson());
        return templateRepository.save(existing);
    }

    @DeleteMapping("/{name}")
    @PreAuthorize("hasAuthority('SCOPE_notifier:admin')")
    @ResponseStatus(HttpStatus.NO_CONTENT)
    public void delete(@PathVariable String name) {
        NotificationTemplate existing = templateRepository.findByName(name)
            .orElseThrow(() -> new ResponseStatusException(HttpStatus.NOT_FOUND,
                "Template not found: " + name));
        templateRepository.delete(existing);
    }
}
