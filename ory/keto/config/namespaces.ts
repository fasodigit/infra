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
