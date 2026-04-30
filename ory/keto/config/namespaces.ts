// SPDX-License-Identifier: AGPL-3.0-or-later
import { Namespace, SubjectSet, Context } from "@ory/keto-namespace-types"

// =============================================================================
// ORY Keto Namespaces (OPL) - FASO DIGITALISATION (DEV)
// =============================================================================

class User implements Namespace {}

class Role implements Namespace {
  related: {
    member: User[]
    owner: User[]
    admin: User[]
  }

  permits = {
    manage: (ctx: Context): boolean =>
      this.related.owner.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),
  }
}

class Platform implements Namespace {
  related: {
    viewer: (User | SubjectSet<Role, "member">)[]
    editor: (User | SubjectSet<Role, "member">)[]
    admin: (User | SubjectSet<Role, "member">)[]
    owner: User[]
    member: (User | SubjectSet<Role, "member">)[]
  }

  permits = {
    view: (ctx: Context): boolean =>
      this.related.viewer.includes(ctx.subject) ||
      this.related.editor.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject) ||
      this.related.owner.includes(ctx.subject),

    edit: (ctx: Context): boolean =>
      this.related.editor.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject) ||
      this.related.owner.includes(ctx.subject),

    manage: (ctx: Context): boolean =>
      this.related.admin.includes(ctx.subject) ||
      this.related.owner.includes(ctx.subject),
  }
}

class Resource implements Namespace {
  related: {
    parent: Platform[]
    viewer: (User | SubjectSet<Role, "member">)[]
    editor: (User | SubjectSet<Role, "member">)[]
    owner: User[]
    admin: (User | SubjectSet<Role, "member">)[]
  }

  permits = {
    view: (ctx: Context): boolean =>
      this.related.viewer.includes(ctx.subject) ||
      this.related.editor.includes(ctx.subject) ||
      this.related.owner.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject) ||
      this.related.parent.traverse((p) => p.permits.view(ctx)),

    edit: (ctx: Context): boolean =>
      this.related.editor.includes(ctx.subject) ||
      this.related.owner.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject) ||
      this.related.parent.traverse((p) => p.permits.edit(ctx)),

    manage: (ctx: Context): boolean =>
      this.related.owner.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject) ||
      this.related.parent.traverse((p) => p.permits.manage(ctx)),

    delete: (ctx: Context): boolean =>
      this.related.owner.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject) ||
      this.related.parent.traverse((p) => p.permits.manage(ctx)),
  }
}

class Department implements Namespace {
  related: {
    member: User[]
    manager: User[]
    admin: User[]
  }

  permits = {
    view: (ctx: Context): boolean =>
      this.related.member.includes(ctx.subject) ||
      this.related.manager.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),

    manage: (ctx: Context): boolean =>
      this.related.manager.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),
  }
}

// ---------------------------------------------------------------------------
// AdminRole — admin-UI Phase 4.b (Stream D2)
// Hiérarchie: super_admin > admin > manager
// Permissions cf. INFRA/docs/GAP-ANALYSIS-PHASE-4A.md §8 (Keto)
// ---------------------------------------------------------------------------
class AdminRole implements Namespace {
  related: {
    super_admin: User[]
    admin: User[]
    manager: User[]
  }

  permits = {
    grant_admin_role: (ctx: Context): boolean =>
      this.related.super_admin.includes(ctx.subject),

    grant_manager_role: (ctx: Context): boolean =>
      this.related.super_admin.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),

    manage_users: (ctx: Context): boolean =>
      this.related.super_admin.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),

    view_audit: (ctx: Context): boolean =>
      this.related.super_admin.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject) ||
      this.related.manager.includes(ctx.subject),

    update_settings: (ctx: Context): boolean =>
      this.related.super_admin.includes(ctx.subject),

    activate_break_glass: (ctx: Context): boolean =>
      this.related.super_admin.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),
  }
}

// ---------------------------------------------------------------------------
// Capability — Delta amendment 2026-04-30 §1
// Fine-grained capability tuples. Each capability key is one Keto object.
// Tuple shape: Capability:<capability_key>#granted@<userId>
// Example   : Capability:audit:view#granted@<uuid>
//             Capability:users:invite#granted@<uuid>
// auth-ms.CapabilityService writes/deletes these tuples on grant/revoke.
// ---------------------------------------------------------------------------
class Capability implements Namespace {
  related: {
    granted: User[]
  }

  permits = {
    has: (ctx: Context): boolean =>
      this.related.granted.includes(ctx.subject),
  }
}

// ---------------------------------------------------------------------------
// TERROIR Phase P0.4 — Multi-tenancy ABAC namespaces
// Voir INFRA/terroir/docs/adr/ADR-006-multi-tenancy.md
// Voir INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md §4 P0.4
//
// Hiérarchie : Tenant → Cooperative → Parcel | HarvestLot
//   * Tenant            : top-level (coopérative cliente, union, exportateur,
//                         bailleur). 1 tenant = 1 schema PG.
//   * Cooperative       : coopérative primaire, fille d'un Tenant.
//   * Parcel            : parcelle agricole, rattachée à une Cooperative.
//   * HarvestLot        : lot de récolte (cacao/café), rattaché à une Coop.
// ---------------------------------------------------------------------------
class Tenant implements Namespace {
  related: {
    member: User[]
    admin: User[]
    agent_terrain: User[]
    gestionnaire: User[]
    exporter: User[]
    bailleur: User[]
  }

  permits = {
    view: (ctx: Context): boolean =>
      this.related.member.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject) ||
      this.related.gestionnaire.includes(ctx.subject) ||
      this.related.exporter.includes(ctx.subject) ||
      this.related.bailleur.includes(ctx.subject),

    manage: (ctx: Context): boolean =>
      this.related.admin.includes(ctx.subject) ||
      this.related.gestionnaire.includes(ctx.subject),

    onboard_member: (ctx: Context): boolean =>
      this.related.gestionnaire.includes(ctx.subject) ||
      this.related.admin.includes(ctx.subject),

    submit_dds: (ctx: Context): boolean =>
      this.related.exporter.includes(ctx.subject),
  }
}

class Cooperative implements Namespace {
  related: {
    parent: Tenant[]
    member_producer: User[]
    agent_collector: User[]
    secretary: User[]
  }

  permits = {
    view: (ctx: Context): boolean =>
      this.related.parent.traverse((t) => t.permits.view(ctx)),

    manage_members: (ctx: Context): boolean =>
      this.related.secretary.includes(ctx.subject) ||
      this.related.parent.traverse((t) => t.permits.manage(ctx)),

    record_harvest: (ctx: Context): boolean =>
      this.related.agent_collector.includes(ctx.subject),
  }
}

class Parcel implements Namespace {
  related: {
    parent: Cooperative[]
    owner_producer: User[]
    editor_agent: User[]
    viewer_supervisor: User[]
  }

  permits = {
    view: (ctx: Context): boolean =>
      this.related.owner_producer.includes(ctx.subject) ||
      this.related.editor_agent.includes(ctx.subject) ||
      this.related.viewer_supervisor.includes(ctx.subject) ||
      this.related.parent.traverse((c) => c.permits.view(ctx)),

    edit_polygon: (ctx: Context): boolean =>
      this.related.editor_agent.includes(ctx.subject),

    submit_eudr: (ctx: Context): boolean =>
      this.related.editor_agent.includes(ctx.subject) ||
      this.related.parent.traverse((c) => c.permits.manage_members(ctx)),
  }
}

class HarvestLot implements Namespace {
  related: {
    parent: Cooperative[]
    creator_agent: User[]
    approver_secretary: User[]
  }

  permits = {
    view: (ctx: Context): boolean =>
      this.related.parent.traverse((c) => c.permits.view(ctx)),

    record: (ctx: Context): boolean =>
      this.related.creator_agent.includes(ctx.subject),

    approve: (ctx: Context): boolean =>
      this.related.approver_secretary.includes(ctx.subject),
  }
}
