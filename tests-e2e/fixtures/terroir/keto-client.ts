// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * KetoClient — wrapper ORY Keto Read API (:4466) + Write API (:4467).
 *
 * Couvre les namespaces P0.D documentés dans
 * `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §4 P0.4 :
 *   - Tenant
 *   - Cooperative
 *   - Parcel
 *   - HarvestLot
 *
 * Endpoints Keto utilisés :
 *   READ  GET    /relation-tuples           (list)
 *   READ  GET    /relation-tuples/check     (check granted)
 *   WRITE PUT    /admin/relation-tuples     (write/upsert)
 *   WRITE DELETE /admin/relation-tuples     (delete)
 *
 * Pas de mocks : on parle aux deux services Keto réels seedés par P0.D.
 */
import { request, type APIRequestContext } from '@playwright/test';

export interface SubjectId {
  /** Subject simple (utilisateur UUID, par exemple). */
  subject_id: string;
}

export interface SubjectSet {
  /** Subject composé : "namespace:object#relation". */
  subject_set: {
    namespace: string;
    object: string;
    relation: string;
  };
}

export type Subject = SubjectId | SubjectSet;

export interface RelationTuple {
  namespace: string;
  object: string;
  relation: string;
  subject_id?: string;
  subject_set?: SubjectSet['subject_set'];
}

export interface CheckResponse {
  allowed: boolean;
}

export interface ListResponse {
  relation_tuples: RelationTuple[];
  next_page_token?: string;
}

export interface KetoCheckRequest {
  namespace: string;
  object: string;
  relation: string;
  subject: Subject;
}

function isSubjectSet(s: Subject): s is SubjectSet {
  return (s as SubjectSet).subject_set !== undefined;
}

export class KetoClient {
  private readonly readURL: string;
  private readonly writeURL: string;

  constructor(opts?: { readURL?: string; writeURL?: string }) {
    this.readURL = opts?.readURL ?? process.env.KETO_READ_URL ?? 'http://localhost:4466';
    this.writeURL = opts?.writeURL ?? process.env.KETO_WRITE_URL ?? 'http://localhost:4467';
  }

  private async api(): Promise<APIRequestContext> {
    return request.newContext({
      extraHTTPHeaders: {
        'content-type': 'application/json',
        accept: 'application/json',
      },
    });
  }

  /** Liste les tuples (Read API) avec filtres optionnels namespace/object. */
  async listTuples(filter?: {
    namespace?: string;
    object?: string;
    relation?: string;
    pageSize?: number;
  }): Promise<RelationTuple[]> {
    const api = await this.api();
    const params = new URLSearchParams();
    if (filter?.namespace) params.set('namespace', filter.namespace);
    if (filter?.object) params.set('object', filter.object);
    if (filter?.relation) params.set('relation', filter.relation);
    params.set('page_size', String(filter?.pageSize ?? 100));
    const res = await api.get(
      `${this.readURL}/relation-tuples?${params.toString()}`,
    );
    if (!res.ok()) {
      throw new Error(
        `Keto list HTTP ${res.status()} : ${await res.text()}`,
      );
    }
    const json = (await res.json()) as ListResponse;
    return json.relation_tuples ?? [];
  }

  /**
   * Vérifie si une relation est accordée. `granted=false` n'est PAS
   * une erreur, c'est juste l'absence de tuple matching (acceptable
   * pour un user inconnu).
   */
  async checkRelation(req: KetoCheckRequest): Promise<boolean> {
    const api = await this.api();
    const params = new URLSearchParams({
      namespace: req.namespace,
      object: req.object,
      relation: req.relation,
    });
    if (isSubjectSet(req.subject)) {
      params.set('subject_set.namespace', req.subject.subject_set.namespace);
      params.set('subject_set.object', req.subject.subject_set.object);
      params.set('subject_set.relation', req.subject.subject_set.relation);
    } else {
      params.set('subject_id', req.subject.subject_id);
    }
    const res = await api.get(
      `${this.readURL}/relation-tuples/check?${params.toString()}`,
    );
    // Keto :4466 returns:
    //   200 {allowed:true}   — relation exists
    //   403 {allowed:false}  — relation absent (NOT a server error,
    //                          it's the negative answer per Keto spec)
    // Both are valid responses. Treat any 4xx that decodes a JSON
    // body containing `allowed:false` as a normal "not granted".
    const status = res.status();
    if (status === 200 || status === 403) {
      try {
        const json = (await res.json()) as CheckResponse;
        return json.allowed === true;
      } catch {
        return false;
      }
    }
    throw new Error(
      `Keto check HTTP ${status} : ${await res.text()}`,
    );
  }

  /**
   * Écrit (upsert) un tuple via la Write API. Renvoie le status HTTP
   * pour permettre aux specs de tester les cas d'erreur (namespace
   * inconnu → 400).
   */
  async writeTuple(tuple: RelationTuple): Promise<{ status: number; body: unknown }> {
    const api = await this.api();
    const res = await api.put(`${this.writeURL}/admin/relation-tuples`, {
      data: tuple,
    });
    let body: unknown;
    try {
      body = await res.json();
    } catch {
      body = await res.text();
    }
    return { status: res.status(), body };
  }

  /** Supprime un tuple (idempotent — 204 même si absent). */
  async deleteTuple(tuple: RelationTuple): Promise<number> {
    const api = await this.api();
    const params = new URLSearchParams({
      namespace: tuple.namespace,
      object: tuple.object,
      relation: tuple.relation,
    });
    if (tuple.subject_id) {
      params.set('subject_id', tuple.subject_id);
    } else if (tuple.subject_set) {
      params.set('subject_set.namespace', tuple.subject_set.namespace);
      params.set('subject_set.object', tuple.subject_set.object);
      params.set('subject_set.relation', tuple.subject_set.relation);
    }
    const res = await api.delete(
      `${this.writeURL}/admin/relation-tuples?${params.toString()}`,
    );
    return res.status();
  }

  async isReachable(): Promise<boolean> {
    try {
      const api = await this.api();
      const r1 = await api.get(`${this.readURL}/health/ready`);
      const r2 = await api.get(`${this.writeURL}/health/ready`);
      return r1.ok() && r2.ok();
    } catch {
      return false;
    }
  }
}
