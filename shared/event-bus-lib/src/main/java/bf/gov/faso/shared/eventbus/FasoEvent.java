// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.shared.eventbus;

import java.time.Instant;
import java.util.Map;

/** Événement canonique FASO — base pour tous les events cross-service. */
public record FasoEvent(
    String id,
    String type,
    String source,
    String tenantId,
    Instant occurredAt,
    Map<String, Object> data
) {}
