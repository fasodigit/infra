/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
 */
package bf.gov.faso.notifier.service;

import bf.gov.faso.notifier.domain.NotificationDelivery;
import bf.gov.faso.notifier.domain.NotificationDelivery.Status;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.stereotype.Repository;

import java.util.List;

/**
 * DeliveryRepository — Spring Data JPA repository for {@link NotificationDelivery}.
 */
@Repository
public interface DeliveryRepository extends JpaRepository<NotificationDelivery, String> {

    Page<NotificationDelivery> findByStatus(Status status, Pageable pageable);

    List<NotificationDelivery> findByStatusAndAttemptsLessThan(Status status, int maxAttempts);

    @Query("SELECT d FROM NotificationDelivery d WHERE d.status IN ('FAILED', 'DLQ') ORDER BY d.createdAt DESC")
    Page<NotificationDelivery> findFailedDeliveries(Pageable pageable);
}
