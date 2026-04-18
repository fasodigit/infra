// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.workflow.impl;

import bf.gov.faso.workflow.activities.PouletsActivities;
import bf.gov.faso.workflow.workflows.OrderWorkflow;
import io.temporal.activity.ActivityOptions;
import io.temporal.common.RetryOptions;
import io.temporal.workflow.Workflow;

import java.time.Duration;
import java.time.Instant;

/**
 * Implémentation de référence OrderWorkflow.
 * Cette classe est chargée par le Worker Temporal au démarrage.
 */
public class OrderWorkflowImpl implements OrderWorkflow {

  private static final ActivityOptions ACTIVITY_OPTS = ActivityOptions.newBuilder()
      .setStartToCloseTimeout(Duration.ofMinutes(2))
      .setRetryOptions(RetryOptions.newBuilder()
          .setInitialInterval(Duration.ofSeconds(1))
          .setBackoffCoefficient(2.0)
          .setMaximumInterval(Duration.ofSeconds(30))
          .setMaximumAttempts(3)
          .build())
      .build();

  private final PouletsActivities activities =
      Workflow.newActivityStub(PouletsActivities.class, ACTIVITY_OPTS);

  // State tracked by queries
  private String phase = "AWAITING_ELEVEUR";
  private String reservationId = null;
  private String chargeId = null;
  private boolean eleveurConfirmed = false;
  private boolean eleveurRejected = false;
  private String eleveurRejectReason = null;
  private boolean clientAccepted = false;
  private boolean disputeRaised = false;
  private String disputeReason = null;

  @Override
  public String processOrder(OrderInput input) {
    activities.auditLog("system", "ORDER_WORKFLOW_START", input.orderId(), null);

    // 1. Notifier l'éleveur
    activities.sendEmail(input.eleveurId(), "order.new", "{\"orderId\":\"" + input.orderId() + "\"}");
    activities.pushNotification(input.eleveurId(), "ORDER_NEW", "Nouvelle commande", input.orderId());

    // 2. Attendre confirmation éleveur (max 24h)
    boolean decided = Workflow.await(Duration.ofHours(24),
        () -> eleveurConfirmed || eleveurRejected || disputeRaised);

    if (!decided || eleveurRejected) {
      phase = eleveurRejected ? "REJECTED_BY_ELEVEUR" : "TIMEOUT";
      activities.sendEmail(input.clientId(), "order.rejected",
          "{\"orderId\":\"" + input.orderId() + "\",\"reason\":\""
              + (eleveurRejectReason != null ? eleveurRejectReason : "timeout") + "\"}");
      return phase;
    }

    if (disputeRaised) {
      phase = "DISPUTED";
      return phase;
    }

    // 3. Réserver stock + paiement
    try {
      phase = "RESERVING";
      reservationId = activities.reservePoulets(input.orderId(), input.lotId(), input.quantity());
      phase = "CHARGING";
      chargeId = activities.chargePayment(input.orderId(), input.paymentMethod(), input.amountFcfa());
      phase = "PAID";
    } catch (Exception ex) {
      // Compensation : release si reservation existe
      if (reservationId != null) activities.releasePoulets(reservationId);
      phase = "FAILED";
      activities.auditLog("system", "ORDER_FAILED", input.orderId(), ex.getMessage());
      return phase;
    }

    // 4. Préparation (timer ~ 2h max)
    Workflow.sleep(Duration.ofHours(2));

    // 5. Planification livraison
    phase = "SCHEDULING_DELIVERY";
    activities.scheduleDelivery(input.orderId(), "auto-assigned", input.desiredDateIso());

    // 6. Marquage livré + réputation + attente acceptation client
    phase = "DELIVERING";
    activities.markDelivered(input.orderId(), Instant.now().toString());

    // Attente acceptation client (max 48h après livraison)
    boolean accepted = Workflow.await(Duration.ofHours(48),
        () -> clientAccepted || disputeRaised);

    if (disputeRaised) {
      phase = "DISPUTED_AFTER_DELIVERY";
      return phase;
    }

    // Auto-acceptation si client silencieux
    activities.updateReputation(input.eleveurId(), 1, "order_" + input.orderId() + "_success");
    phase = "COMPLETED";
    activities.sendEmail(input.clientId(), "order.completed",
        "{\"orderId\":\"" + input.orderId() + "\"}");
    activities.auditLog("system", "ORDER_COMPLETED", input.orderId(), null);
    return phase;
  }

  @Override public void eleveurConfirms(String eleveurId) { this.eleveurConfirmed = true; }
  @Override public void eleveurRejects(String reason) {
    this.eleveurRejected = true;
    this.eleveurRejectReason = reason;
  }
  @Override public void clientAcceptsDelivery() { this.clientAccepted = true; }
  @Override public void raiseDispute(String reason) {
    this.disputeRaised = true;
    this.disputeReason = reason;
  }

  @Override
  public OrderState getState() {
    return new OrderState(
        phase,
        "current",
        reservationId,
        chargeId,
        Instant.now().toString()
    );
  }
}
