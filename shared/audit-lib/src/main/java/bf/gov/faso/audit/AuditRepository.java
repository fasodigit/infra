// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit;

import org.springframework.data.jpa.repository.JpaRepository;

/**
 * Spring Data repository for {@link AuditEvent}.
 *
 * <p>Only {@code save()} should be used in production — the underlying table
 * is append-only (UPDATE/DELETE blocked by DB triggers).
 */
public interface AuditRepository extends JpaRepository<AuditEvent, Long> {
}
