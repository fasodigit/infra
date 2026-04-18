// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.workflow.activities;

import io.temporal.activity.ActivityInterface;
import io.temporal.activity.ActivityMethod;

/**
 * Activities centralisées — appelées par les 6 workflows Poulets.
 * Chaque méthode correspond à un side-effect idempotent et retryable.
 *
 * Retry policy par défaut : 3 tentatives, backoff exponentiel (1s → 30s).
 * Idempotence : utiliser `idempotencyKey` = `{workflowId}:{step}`.
 */
@ActivityInterface
public interface PouletsActivities {

  // --------------------------------------------------------- Notifications
  @ActivityMethod void sendEmail(String to, String templateKey, String dataJson);
  @ActivityMethod void sendSms(String phone, String message);
  @ActivityMethod void pushNotification(String userId, String type, String title, String body);

  // --------------------------------------------------------- Orders
  @ActivityMethod String reservePoulets(String orderId, String lotId, int quantity);
  @ActivityMethod void releasePoulets(String reservationId);
  @ActivityMethod String chargePayment(String orderId, String paymentMethod, long amountFcfa);
  @ActivityMethod void refundPayment(String chargeId, long amountFcfa);
  @ActivityMethod void scheduleDelivery(String orderId, String livreurId, String dateTimeIso);
  @ActivityMethod void markDelivered(String orderId, String deliveredAtIso);

  // --------------------------------------------------------- Reputation
  @ActivityMethod void updateReputation(String userId, int delta, String reason);
  @ActivityMethod void revokeReputationPoint(String userId, String reason);

  // --------------------------------------------------------- Impression / Docs
  @ActivityMethod String callEcCertificateRenderer(String templateName, String dataJson);
  @ActivityMethod String archiveToWorm(String documentId, String pdfBase64);

  // --------------------------------------------------------- MFA
  @ActivityMethod boolean checkMfaStatus(String userId);
  @ActivityMethod void sendMfaReminder(String userId, int attempt);
  @ActivityMethod void lockAccount(String userId, String reason);

  // --------------------------------------------------------- Halal
  @ActivityMethod void emitHalalStepEvent(String lotId, int step, String status);
  @ActivityMethod String generateHalalQrCode(String certifId);

  // --------------------------------------------------------- Growth (lots)
  @ActivityMethod void recordPeseeReminder(String lotId, int week);
  @ActivityMethod void alertAdminAbsence(String lotId, String eleveurId, int missedWeeks);

  // --------------------------------------------------------- Dispute
  @ActivityMethod String reserveEscrow(String orderId, long amountFcfa);
  @ActivityMethod void releaseEscrow(String escrowId, String beneficiary);

  // --------------------------------------------------------- Announces
  @ActivityMethod String moderationScan(String annonceId, String contentJson);
  @ActivityMethod void publishAnnounce(String annonceId);
  @ActivityMethod void removeAnnounce(String annonceId, String reason);

  // --------------------------------------------------------- Audit
  @ActivityMethod void auditLog(String actorId, String action, String resource, String detail);
}
