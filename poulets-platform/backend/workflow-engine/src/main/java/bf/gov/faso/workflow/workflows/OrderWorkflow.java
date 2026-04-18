// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.workflow.workflows;

import io.temporal.workflow.QueryMethod;
import io.temporal.workflow.SignalMethod;
import io.temporal.workflow.WorkflowInterface;
import io.temporal.workflow.WorkflowMethod;

/**
 * OrderWorkflow — lifecycle d'une commande client→éleveur.
 * Durée : 1 à 7 jours.
 *
 * Étapes :
 *   1. Notification éleveur (email + push)
 *   2. Attente confirmation éleveur (timer 24h)
 *   3. Réservation stock + paiement escrow
 *   4. Préparation (timer variable selon éleveur)
 *   5. Planification livraison
 *   6. Livraison → markDelivered → updateReputation
 *   7. Compensation si échec paiement ou annulation
 */
@WorkflowInterface
public interface OrderWorkflow {

  /** Démarre le workflow. Retourne état final (COMPLETED / CANCELED / FAILED). */
  @WorkflowMethod
  String processOrder(OrderInput input);

  /** Éleveur confirme la commande → débloque la suite. */
  @SignalMethod
  void eleveurConfirms(String eleveurId);

  /** Éleveur rejette la commande → compensation. */
  @SignalMethod
  void eleveurRejects(String reason);

  /** Client accepte la livraison → finalise. */
  @SignalMethod
  void clientAcceptsDelivery();

  /** Client ouvre un dispute → annule et lance DisputeSaga. */
  @SignalMethod
  void raiseDispute(String reason);

  /** Lecture sync de l'état courant (pour UI admin). */
  @QueryMethod
  OrderState getState();

  // --------------------------------------------------------- DTOs

  record OrderInput(
      String orderId,
      String clientId,
      String eleveurId,
      String lotId,
      int quantity,
      long amountFcfa,
      String paymentMethod,
      String deliveryAddress,
      String desiredDateIso
  ) {}

  record OrderState(
      String phase,           // AWAITING_ELEVEUR, RESERVED, PAID, SHIPPED, DELIVERED, CANCELED, FAILED
      String orderId,
      String reservationId,
      String chargeId,
      String lastUpdateIso
  ) {}
}
