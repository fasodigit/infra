import { Injectable, inject } from '@angular/core';
import { Apollo } from 'apollo-angular';
import { Observable, map } from 'rxjs';

import {
  GET_POULETS,
  GET_POULET_BY_ID,
  GET_MES_POULETS,
  GET_MES_COMMANDES,
  GET_ELEVEUR_STATS,
  CREATE_POULET,
  UPDATE_POULET,
  DELETE_POULET,
  PASSER_COMMANDE,
  Poulet,
  Commande,
  PouletFilter,
  Page,
  EleveurStats,
  CreatePouletInput,
  UpdatePouletInput,
  CommandeInput,
} from './graphql.service';

@Injectable({ providedIn: 'root' })
export class PouletService {
  private readonly apollo = inject(Apollo);

  // =========================================================================
  // Queries
  // =========================================================================

  /**
   * Fetch paginated poulets with optional filters.
   */
  getPoulets(
    filter?: PouletFilter,
    page: number = 0,
    size: number = 12,
  ): Observable<Page<Poulet>> {
    return this.apollo
      .watchQuery<{ poulets: Page<Poulet> }>({
        query: GET_POULETS,
        variables: { filter, page, size },
      })
      .valueChanges.pipe(map((result) => result.data.poulets));
  }

  /**
   * Fetch a single poulet by ID.
   */
  getPouletById(id: string): Observable<Poulet> {
    return this.apollo
      .watchQuery<{ poulet: Poulet }>({
        query: GET_POULET_BY_ID,
        variables: { id },
      })
      .valueChanges.pipe(map((result) => result.data.poulet));
  }

  /**
   * Fetch current eleveur's poulets (authenticated).
   */
  getMesPoulets(page: number = 0, size: number = 20): Observable<Page<Poulet>> {
    return this.apollo
      .watchQuery<{ mesPoulets: Page<Poulet> }>({
        query: GET_MES_POULETS,
        variables: { page, size },
      })
      .valueChanges.pipe(map((result) => result.data.mesPoulets));
  }

  /**
   * Fetch current user's orders (authenticated).
   */
  getMesCommandes(page: number = 0, size: number = 20): Observable<Page<Commande>> {
    return this.apollo
      .watchQuery<{ mesCommandes: Page<Commande> }>({
        query: GET_MES_COMMANDES,
        variables: { page, size },
      })
      .valueChanges.pipe(map((result) => result.data.mesCommandes));
  }

  /**
   * Fetch eleveur dashboard statistics.
   */
  getEleveurStats(): Observable<EleveurStats> {
    return this.apollo
      .watchQuery<{ eleveurStats: EleveurStats }>({
        query: GET_ELEVEUR_STATS,
      })
      .valueChanges.pipe(map((result) => result.data.eleveurStats));
  }

  // =========================================================================
  // Mutations
  // =========================================================================

  /**
   * Create a new poulet listing (eleveur only).
   */
  createPoulet(input: CreatePouletInput): Observable<Poulet> {
    return this.apollo
      .mutate<{ createPoulet: Poulet }>({
        mutation: CREATE_POULET,
        variables: { input },
        refetchQueries: [{ query: GET_MES_POULETS }],
      })
      .pipe(map((result) => result.data!.createPoulet));
  }

  /**
   * Update an existing poulet listing (eleveur only).
   */
  updatePoulet(id: string, input: UpdatePouletInput): Observable<Poulet> {
    return this.apollo
      .mutate<{ updatePoulet: Poulet }>({
        mutation: UPDATE_POULET,
        variables: { id, input },
      })
      .pipe(map((result) => result.data!.updatePoulet));
  }

  /**
   * Delete a poulet listing (eleveur only).
   */
  deletePoulet(id: string): Observable<boolean> {
    return this.apollo
      .mutate<{ deletePoulet: boolean }>({
        mutation: DELETE_POULET,
        variables: { id },
        refetchQueries: [{ query: GET_MES_POULETS }],
      })
      .pipe(map((result) => result.data!.deletePoulet));
  }

  /**
   * Place an order for a poulet (client).
   */
  passerCommande(input: CommandeInput): Observable<Commande> {
    return this.apollo
      .mutate<{ passerCommande: Commande }>({
        mutation: PASSER_COMMANDE,
        variables: { input },
        refetchQueries: [{ query: GET_MES_COMMANDES }],
      })
      .pipe(map((result) => result.data!.passerCommande));
  }
}
