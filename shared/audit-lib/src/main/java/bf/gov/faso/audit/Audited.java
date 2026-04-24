// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit;

import java.lang.annotation.*;

/**
 * Marks a method for automatic audit logging via {@link AuditAspect}.
 *
 * <p>Usage:
 * <pre>
 * {@literal @}Audited(action = "CREATE_ORDER", resourceType = "Commande")
 * public Commande createOrder(OrderRequest req) { ... }
 * </pre>
 */
@Target(ElementType.METHOD)
@Retention(RetentionPolicy.RUNTIME)
@Documented
public @interface Audited {

    /** The action being performed (e.g. CREATE_ORDER, LOGIN, DELETE_USER). */
    String action();

    /** The type of resource being acted upon (e.g. Commande, User, Poulet). */
    String resourceType();
}
