// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.workflow.workflows;

import io.temporal.workflow.QueryMethod;
import io.temporal.workflow.SignalMethod;
import io.temporal.workflow.WorkflowInterface;
import io.temporal.workflow.WorkflowMethod;

/**
 * DisputeSaga — médiation compensatoire entre client et éleveur.
 * Durée : 3 à 14 jours.
 *
 * Saga pattern avec 2 branches compensatoires :
 *
 *   1. Réserve escrow du paiement client
 *   2. Investigation (admin assigné)
 *   3. Four-eyes : 2 admins doivent trancher (REFUND ou UPHOLD)
 *   4a. REFUND → refundClient + releaseEscrow + revokeReputationPoint
 *   4b. UPHOLD → releaseEscrowToEleveur + notifyClient
 *   5. En cas d'erreur : releaseEscrowToNeutral (fallback)
 */
@WorkflowInterface
public interface DisputeSaga {

  @WorkflowMethod
  DisputeResult resolve(DisputeInput input);

  /** Admin dépose sa décision (REFUND ou UPHOLD) avec motif. */
  @SignalMethod
  void adminDecision(String adminId, Decision decision, String comment);

  @QueryMethod
  DisputeState getState();

  enum Decision { REFUND, UPHOLD }

  record DisputeInput(
      String disputeId,
      String orderId,
      String clientId,
      String eleveurId,
      long amountFcfa,
      String clientComplaint
  ) {}

  record DisputeState(
      String disputeId,
      String phase,   // ESCROW_RESERVED, AWAITING_DECISIONS, COMPENSATING, RESOLVED
      java.util.List<String> decisions,
      String finalDecision,
      String lastUpdateIso
  ) {}

  record DisputeResult(
      String disputeId,
      String finalDecision,
      long refundedFcfa,
      long paidToEleveurFcfa,
      String resolvedAtIso
  ) {}
}
