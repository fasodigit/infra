// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.workflow.workflows;

import io.temporal.workflow.QueryMethod;
import io.temporal.workflow.SignalMethod;
import io.temporal.workflow.WorkflowInterface;
import io.temporal.workflow.WorkflowMethod;

/**
 * MfaOnboardingWorkflow — relances MFA post-inscription.
 * Durée : 7 à 30 jours.
 *
 * Timers :
 *   J+3  → 1ère relance email
 *   J+7  → 2nde relance + snooze notifications UI
 *   J+30 → verrouillage compte si MFA toujours incomplet
 *
 * Se termine si `mfaCompleted` signal reçu ou compte verrouillé.
 */
@WorkflowInterface
public interface MfaOnboardingWorkflow {

  @WorkflowMethod
  String startOnboarding(String userId);

  /** L'utilisateur a complété MFA → termine immédiatement. */
  @SignalMethod
  void mfaCompleted();

  /** L'utilisateur a partiellement configuré (au moins 1 méthode) — track progress. */
  @SignalMethod
  void mfaPartial(String method);

  @QueryMethod
  MfaOnboardingState getState();

  record MfaOnboardingState(
      String userId,
      int remindersSent,
      boolean completed,
      boolean locked,
      String nextReminderIso
  ) {}
}
