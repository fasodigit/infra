import { Injectable, inject } from '@angular/core';
import { Apollo } from 'apollo-angular';
import { Observable, map, filter as rxFilter } from 'rxjs';
import {
  Annonce,
  Besoin,
  MatchResult,
  AnnonceFilter,
  BesoinFilter,
  CreateAnnonceInput,
  CreateBesoinInput,
} from '../../../shared/models/marketplace.models';
import { Page } from '../../../services/graphql.service';
import {
  GET_ANNONCES,
  GET_ANNONCE_BY_ID,
  GET_SIMILAR_ANNONCES,
  CREATE_ANNONCE,
  GET_BESOINS,
  GET_BESOIN_BY_ID,
  CREATE_BESOIN,
  GET_MATCHES_FOR_ELEVEUR,
  GET_MATCHES_FOR_CLIENT,
} from '../graphql/marketplace.graphql';

@Injectable({ providedIn: 'root' })
export class MarketplaceService {
  private readonly apollo = inject(Apollo);

  // ---- Annonces ----

  getAnnonces(filter?: AnnonceFilter, page = 0, size = 12): Observable<Page<Annonce>> {
    return this.apollo
      .watchQuery<{ annonces: Page<Annonce> }>({
        query: GET_ANNONCES,
        variables: { filter, page, size },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.annonces as Page<Annonce>),
      );
  }

  getAnnonceById(id: string): Observable<Annonce> {
    return this.apollo
      .watchQuery<{ annonce: Annonce }>({
        query: GET_ANNONCE_BY_ID,
        variables: { id },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.annonce as Annonce),
      );
  }

  getSimilarAnnonces(annonceId: string, limit = 6): Observable<Annonce[]> {
    return this.apollo
      .watchQuery<{ similarAnnonces: Annonce[] }>({
        query: GET_SIMILAR_ANNONCES,
        variables: { annonceId, limit },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.similarAnnonces as Annonce[]),
      );
  }

  createAnnonce(input: CreateAnnonceInput): Observable<Annonce> {
    return this.apollo
      .mutate<{ createAnnonce: Annonce }>({
        mutation: CREATE_ANNONCE,
        variables: { input },
        refetchQueries: [{ query: GET_ANNONCES }],
      })
      .pipe(map((r) => r.data!.createAnnonce));
  }

  // ---- Besoins ----

  getBesoins(filter?: BesoinFilter, page = 0, size = 12): Observable<Page<Besoin>> {
    return this.apollo
      .watchQuery<{ besoins: Page<Besoin> }>({
        query: GET_BESOINS,
        variables: { filter, page, size },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.besoins as Page<Besoin>),
      );
  }

  getBesoinById(id: string): Observable<Besoin> {
    return this.apollo
      .watchQuery<{ besoin: Besoin }>({
        query: GET_BESOIN_BY_ID,
        variables: { id },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.besoin as Besoin),
      );
  }

  createBesoin(input: CreateBesoinInput): Observable<Besoin> {
    return this.apollo
      .mutate<{ createBesoin: Besoin }>({
        mutation: CREATE_BESOIN,
        variables: { input },
        refetchQueries: [{ query: GET_BESOINS }],
      })
      .pipe(map((r) => r.data!.createBesoin));
  }

  // ---- Matching ----

  getMatchesForEleveur(page = 0, size = 20): Observable<Page<MatchResult>> {
    return this.apollo
      .watchQuery<{ matchesForEleveur: Page<MatchResult> }>({
        query: GET_MATCHES_FOR_ELEVEUR,
        variables: { page, size },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.matchesForEleveur as Page<MatchResult>),
      );
  }

  getMatchesForClient(page = 0, size = 20): Observable<Page<MatchResult>> {
    return this.apollo
      .watchQuery<{ matchesForClient: Page<MatchResult> }>({
        query: GET_MATCHES_FOR_CLIENT,
        variables: { page, size },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.matchesForClient as Page<MatchResult>),
      );
  }
}
