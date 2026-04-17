/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.controller;

import bf.gov.faso.notifier.domain.NotificationDelivery;
import bf.gov.faso.notifier.domain.NotificationDelivery.Status;
import bf.gov.faso.notifier.service.DeliveryRepository;
import bf.gov.faso.notifier.service.NotificationService;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.HttpStatus;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

/**
 * DeliveryController — list deliveries and trigger manual retry.
 *
 * <p>Endpoint: {@code /api/deliveries}
 */
@RestController
@RequestMapping("/api/deliveries")
public class DeliveryController {

    private final DeliveryRepository deliveryRepository;
    private final NotificationService notificationService;
    private final ObjectMapper objectMapper;

    public DeliveryController(
            DeliveryRepository deliveryRepository,
            NotificationService notificationService,
            ObjectMapper objectMapper) {
        this.deliveryRepository = deliveryRepository;
        this.notificationService = notificationService;
        this.objectMapper = objectMapper;
    }

    @GetMapping
    @PreAuthorize("hasAuthority('SCOPE_notifier:read')")
    public Page<NotificationDelivery> list(
            @RequestParam(required = false) Status status,
            Pageable pageable) {
        if (status != null) {
            return deliveryRepository.findByStatus(status, pageable);
        }
        return deliveryRepository.findAll(pageable);
    }

    @GetMapping("/{deliveryId}")
    @PreAuthorize("hasAuthority('SCOPE_notifier:read')")
    public NotificationDelivery get(@PathVariable String deliveryId) {
        return deliveryRepository.findById(deliveryId)
            .orElseThrow(() -> new ResponseStatusException(HttpStatus.NOT_FOUND,
                "Delivery not found: " + deliveryId));
    }

    @PostMapping("/{deliveryId}/retry")
    @PreAuthorize("hasAuthority('SCOPE_notifier:admin')")
    @ResponseStatus(HttpStatus.ACCEPTED)
    public NotificationDelivery retry(@PathVariable String deliveryId) throws Exception {
        NotificationDelivery delivery = deliveryRepository.findById(deliveryId)
            .orElseThrow(() -> new ResponseStatusException(HttpStatus.NOT_FOUND,
                "Delivery not found: " + deliveryId));

        if (delivery.getStatus() == Status.SENT) {
            throw new ResponseStatusException(HttpStatus.CONFLICT,
                "Delivery already successfully sent");
        }

        // Reset status to PENDING for retry
        delivery.setStatus(Status.PENDING);
        delivery.setLastError(null);
        delivery = deliveryRepository.save(delivery);

        // Re-parse stored event payload and dispatch
        var payload = objectMapper.readValue(
            delivery.getEventPayload(),
            bf.gov.faso.notifier.domain.GithubEventPayload.class);
        notificationService.dispatch(
            delivery.getDeliveryId(),
            delivery.getRecipient(),
            delivery.getTemplateName(),
            payload,
            delivery.getEventPayload().getBytes());

        return deliveryRepository.findById(deliveryId).orElse(delivery);
    }

    @GetMapping("/failed")
    @PreAuthorize("hasAuthority('SCOPE_notifier:read')")
    public Page<NotificationDelivery> listFailed(Pageable pageable) {
        return deliveryRepository.findFailedDeliveries(pageable);
    }
}
