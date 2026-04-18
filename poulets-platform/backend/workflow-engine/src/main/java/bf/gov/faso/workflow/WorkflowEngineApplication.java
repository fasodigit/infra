// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

package bf.gov.faso.workflow;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Workflow Engine Poulets BF — host pour worker Temporal.
 * Port HTTP : 8902 (admin endpoints pour UI /admin/workflows).
 * Task queues : poulets-main.
 */
@SpringBootApplication
public class WorkflowEngineApplication {
  public static void main(String[] args) {
    SpringApplication.run(WorkflowEngineApplication.class, args);
  }
}
