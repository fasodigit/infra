// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.workflow.workflows;

import io.temporal.workflow.QueryMethod;
import io.temporal.workflow.SignalMethod;
import io.temporal.workflow.WorkflowInterface;
import io.temporal.workflow.WorkflowMethod;

/**
 * HalalCertificationWorkflow — 6 étapes de certification halal d'un lot.
 * Durée : 2 à 30 jours.
 *
 * Étapes :
 *   1. Élevage halal conforme
 *   2. Identification du lot
 *   3. Abattoir agréé halal
 *   4. Présence sacrificateur
 *   5. Contrôle vétérinaire
 *   6. Émission certificat + QR code
 *
 * Chaque étape nécessite un signal admin. Étape 6 déclenche un four-eyes
 * (2 admins doivent approuver) avant génération du PDF via
 * ec-certificate-renderer.
 */
@WorkflowInterface
public interface HalalCertificationWorkflow {

  @WorkflowMethod
  HalalResult process(HalalInput input);

  /** Admin approuve l'étape N du process. */
  @SignalMethod
  void adminApproveStep(int step, String adminId);

  /** Admin rejette l'étape N avec motif. */
  @SignalMethod
  void adminRejectStep(int step, String adminId, String reason);

  /** Approbation four-eyes (étape 6 uniquement). */
  @SignalMethod
  void fourEyesApprove(String adminId);

  @QueryMethod
  HalalProgress getProgress();

  record HalalInput(
      String lotId,
      String eleveurId,
      int quantity,
      String race,
      String abattoirId,
      String sacrificateurId,
      String veterinaireId
  ) {}

  record HalalResult(String certifId, String qrCodeUrl, String status) {}

  record HalalProgress(
      int currentStep,
      int totalSteps,
      java.util.List<String> approvedSteps,
      java.util.List<String> fourEyesApprovals,
      String lastUpdateIso
  ) {}
}
