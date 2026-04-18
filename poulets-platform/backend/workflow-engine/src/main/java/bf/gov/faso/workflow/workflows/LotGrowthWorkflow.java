// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.workflow.workflows;

import io.temporal.workflow.QueryMethod;
import io.temporal.workflow.SignalMethod;
import io.temporal.workflow.WorkflowInterface;
import io.temporal.workflow.WorkflowMethod;

/**
 * LotGrowthWorkflow — suivi croissance d'un lot avicole.
 * Durée : 45 à 60 jours.
 *
 * Chaque semaine :
 *   - Timer 7j → sendPeseeReminder à l'éleveur
 *   - Attente signal `peseeRecorded` (timeout 3j)
 *   - Si 2 semaines consécutives sans pesée → alertAdminAbsence
 *
 * À J+60 (ou signal `lotClosed`) → termine et archive les stats.
 *
 * Utilise Continue-as-New si workflow dépasse 1000 events (évite history infini).
 */
@WorkflowInterface
public interface LotGrowthWorkflow {

  @WorkflowMethod
  LotGrowthResult trackLot(LotInput input);

  /** Éleveur enregistre une pesée (poids moyen en grammes pour l'âge). */
  @SignalMethod
  void peseeRecorded(int ageDays, int weightGrams);

  /** Lot vendu/clos par l'éleveur → termine. */
  @SignalMethod
  void lotClosed(String reason);

  @QueryMethod
  LotGrowthState getState();

  record LotInput(String lotId, String eleveurId, int quantity, String race, String startDateIso) {}

  record LotGrowthState(
      String lotId,
      int currentWeek,
      int totalPesees,
      int missedWeeks,
      int lastWeightGrams,
      String lastPeseeIso
  ) {}

  record LotGrowthResult(
      String lotId,
      int totalPesees,
      int avgFinalWeight,
      String closedAtIso,
      String status
  ) {}
}
