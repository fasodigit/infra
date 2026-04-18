// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.workflow.workflows;

import io.temporal.workflow.QueryMethod;
import io.temporal.workflow.SignalMethod;
import io.temporal.workflow.WorkflowInterface;
import io.temporal.workflow.WorkflowMethod;

/**
 * AnnouncePublishWorkflow — publication d'une annonce marketplace.
 * Durée : < 5 minutes (rapide) à 24h si modération humaine requise.
 *
 * Étapes :
 *   1. Scan modération automatique (contenu + images)
 *   2. Si flag → escalade vers file de modération humaine
 *   3. Signal adminDecision (approve/reject)
 *   4. Publication ou suppression + notification éleveur
 */
@WorkflowInterface
public interface AnnouncePublishWorkflow {

  @WorkflowMethod
  String publish(AnnounceInput input);

  /** Modérateur approuve l'annonce. */
  @SignalMethod
  void moderatorApprove(String moderatorId);

  /** Modérateur refuse avec motif. */
  @SignalMethod
  void moderatorReject(String moderatorId, String reason);

  @QueryMethod
  AnnounceState getState();

  record AnnounceInput(
      String annonceId,
      String eleveurId,
      String race,
      int quantity,
      long pricePerKgFcfa,
      String region,
      String description,
      java.util.List<String> photoUrls
  ) {}

  record AnnounceState(
      String annonceId,
      String phase,      // SCANNING, AUTO_APPROVED, AWAITING_HUMAN, PUBLISHED, REJECTED
      boolean humanRequired,
      String moderatorId,
      String lastUpdateIso
  ) {}
}
