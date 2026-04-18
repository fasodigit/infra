// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

export interface Review {
  id: string;
  breederId: string;
  clientId: string;
  clientName: string;
  clientAvatar?: string | null;
  rating: number;
  comment: string;
  photos?: string[];
  createdAt: string;
  orderId?: string;
}

export interface ReviewStats {
  breederId: string;
  average: number;
  total: number;
  /** Count by star bucket, index 0 = 1★ … index 4 = 5★ */
  distribution: [number, number, number, number, number];
}

export interface BreederProfile {
  id: string;
  name: string;
  prenom?: string;
  avatar?: string | null;
  coverPhoto?: string | null;
  region: string;
  city?: string;
  latitude?: number;
  longitude?: number;
  distanceKm?: number;
  bio?: string;
  specialties: string[];
  halalCertified: boolean;
  veterinaryVerified: boolean;
  bioCertified: boolean;
  memberSince: string;
  responseTimeHours?: number;
  totalSales?: number;
  gallery?: string[];
  phone?: string;
  whatsapp?: string;
  email?: string;
}
