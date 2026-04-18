// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

export type ModerationType =
  | 'ANNONCE_NEW'
  | 'ANNONCE_FLAGGED'
  | 'HALAL_CERT_REVIEW'
  | 'USER_REPORT'
  | 'REVIEW_FLAGGED';

export type ModerationStatus = 'pending' | 'in_review' | 'approved' | 'rejected' | 'escalated';

export type Priority = 'P0' | 'P1' | 'P2';

export interface ModerationAttachment {
  id: string;
  name: string;
  mime: string;
  url: string;
  /** Pré-aperçu (image ou PDF) ou null si texte. */
  previewUrl?: string;
}

export interface ModerationItem {
  id: string;
  type: ModerationType;
  priority: Priority;
  status: ModerationStatus;
  title: string;
  summary: string;
  authorId: string;
  authorName: string;
  region?: string;
  createdAt: string;
  /** SLA restant en minutes. */
  slaRemainingMin: number;
  lockedBy?: string;
  lockedUntil?: string;
  attachments?: ModerationAttachment[];
  history?: ModerationEvent[];
  requiresFourEyes?: boolean;
  fourEyesApprovals?: { adminId: string; adminName: string; at: string }[];
}

export interface ModerationEvent {
  at: string;
  actorName: string;
  action: 'create' | 'lock' | 'unlock' | 'approve' | 'reject' | 'escalate' | 'comment' | 'four-eyes-approve';
  comment?: string;
}
