/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.domain;

import jakarta.persistence.*;
import jakarta.validation.constraints.Email;
import jakarta.validation.constraints.NotBlank;

import java.time.Instant;

/**
 * NotificationDelivery — tracks lifecycle of a single outbound email.
 *
 * <p>Status transitions: PENDING → SENT (happy path)
 *                        PENDING → FAILED → FAILED (retry exhausted) → DLQ
 */
@Entity
@Table(name = "notification_deliveries",
       indexes = {
           @Index(name = "idx_delivery_status", columnList = "status"),
           @Index(name = "idx_delivery_template", columnList = "template_name"),
           @Index(name = "idx_delivery_recipient", columnList = "recipient")
       })
public class NotificationDelivery {

    public enum Status {
        PENDING, SENT, FAILED, DLQ
    }

    @Id
    @Column(name = "delivery_id", nullable = false, updatable = false, length = 64)
    private String deliveryId;

    @Email
    @NotBlank
    @Column(name = "recipient", nullable = false, length = 320)
    private String recipient;

    @NotBlank
    @Column(name = "template_name", nullable = false, length = 128)
    private String templateName;

    @Enumerated(EnumType.STRING)
    @Column(name = "status", nullable = false, length = 16)
    private Status status = Status.PENDING;

    @Column(name = "attempts", nullable = false)
    private int attempts = 0;

    @Column(name = "last_error", columnDefinition = "TEXT")
    private String lastError;

    @Column(name = "sent_at")
    private Instant sentAt;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @Column(name = "updated_at")
    private Instant updatedAt;

    /** Raw event payload (JSON) for DLQ replay / audit. */
    @Column(name = "event_payload", columnDefinition = "TEXT")
    private String eventPayload;

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
        updatedAt = createdAt;
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }

    // ── Getters / Setters ──────────────────────────────────────────

    public String getDeliveryId() { return deliveryId; }
    public void setDeliveryId(String deliveryId) { this.deliveryId = deliveryId; }

    public String getRecipient() { return recipient; }
    public void setRecipient(String recipient) { this.recipient = recipient; }

    public String getTemplateName() { return templateName; }
    public void setTemplateName(String templateName) { this.templateName = templateName; }

    public Status getStatus() { return status; }
    public void setStatus(Status status) { this.status = status; }

    public int getAttempts() { return attempts; }
    public void setAttempts(int attempts) { this.attempts = attempts; }

    public String getLastError() { return lastError; }
    public void setLastError(String lastError) { this.lastError = lastError; }

    public Instant getSentAt() { return sentAt; }
    public void setSentAt(Instant sentAt) { this.sentAt = sentAt; }

    public Instant getCreatedAt() { return createdAt; }
    public Instant getUpdatedAt() { return updatedAt; }

    public String getEventPayload() { return eventPayload; }
    public void setEventPayload(String eventPayload) { this.eventPayload = eventPayload; }
}
