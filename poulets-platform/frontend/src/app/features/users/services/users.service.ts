// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable } from '@angular/core';
import { Observable, delay, of } from 'rxjs';
import { PlatformUser } from '@shared/models/admin.models';
import { UserRole } from '@core/config/project-config.token';

export interface CreateUserInput {
  role: UserRole;
  email: string;
  firstName: string;
  lastName: string;
  phone?: string;
  region?: string;
  roleMeta?: Record<string, unknown>;
  sendInvitation?: boolean;
}

export interface UserListResult {
  content: PlatformUser[];
  totalElements: number;
  totalPages: number;
}

@Injectable({ providedIn: 'root' })
export class UsersService {
  private users: PlatformUser[] = generateMockUsers();

  list(page = 0, size = 20, role?: UserRole, search = ''): Observable<UserListResult> {
    const q = search.trim().toLowerCase();
    const filtered = this.users.filter((u) => {
      if (role && u.role !== role) return false;
      if (q) {
        const blob = `${u.displayName} ${u.email} ${u.phone ?? ''} ${u.region ?? ''}`.toLowerCase();
        if (!blob.includes(q)) return false;
      }
      return true;
    });
    const start = page * size;
    return of({
      content: filtered.slice(start, start + size),
      totalElements: filtered.length,
      totalPages: Math.ceil(filtered.length / size),
    }).pipe(delay(200));
  }

  get(id: string): Observable<PlatformUser | null> {
    return of(this.users.find((u) => u.id === id) ?? null).pipe(delay(150));
  }

  create(input: CreateUserInput): Observable<{ user: PlatformUser; invitationLink: string }> {
    const id = 'u-' + Math.random().toString(36).slice(2, 8);
    const user: PlatformUser = {
      id,
      email: input.email,
      firstName: input.firstName,
      lastName: input.lastName,
      displayName: `${input.firstName} ${input.lastName}`.trim(),
      role: input.role,
      phone: input.phone,
      region: input.region,
      isActive: true,
      mfaConfigured: false,
      mfaStatus: { email: true, passkey: false, totp: false, backupCodes: false, phone: false },
      createdAt: new Date().toISOString(),
      roleMeta: input.roleMeta,
    };
    this.users = [user, ...this.users];
    return of({
      user,
      invitationLink: `https://poulets.fasodigitalisation.bf/auth/accept-invite?token=${id}`,
    }).pipe(delay(400));
  }

  update(id: string, patch: Partial<PlatformUser>): Observable<PlatformUser | null> {
    this.users = this.users.map((u) => u.id === id ? { ...u, ...patch } : u);
    return of(this.users.find((u) => u.id === id) ?? null).pipe(delay(200));
  }

  deactivate(id: string): Observable<PlatformUser | null> {
    return this.update(id, { isActive: false });
  }

  reactivate(id: string): Observable<PlatformUser | null> {
    return this.update(id, { isActive: true });
  }

  forceLogout(id: string): Observable<boolean> {
    // Stub: would call BFF → Kratos /admin/identities/{id}/sessions DELETE
    return of(true).pipe(delay(150));
  }
}

function generateMockUsers(): PlatformUser[] {
  const sample: Array<Partial<PlatformUser>> = [
    { firstName: 'Kassim',  lastName: 'Ouédraogo', role: 'ELEVEUR',    region: 'Centre',        phone: '+22670112233', mfaStatus: { email: true, passkey: true,  totp: true,  backupCodes: true, phone: false } },
    { firstName: 'Awa',     lastName: 'Sankara',   role: 'ELEVEUR',    region: 'Hauts-Bassins', phone: '+22670223344', mfaStatus: { email: true, passkey: true,  totp: false, backupCodes: true, phone: true  } },
    { firstName: 'Oumar',   lastName: 'Traoré',    role: 'ELEVEUR',    region: 'Centre-Ouest',  phone: '+22670334455', mfaStatus: { email: true, passkey: false, totp: true,  backupCodes: true, phone: false } },
    { firstName: 'Fatim',   lastName: 'Compaoré',  role: 'CLIENT',     region: 'Nord',          phone: '+22670445566', mfaStatus: { email: true, passkey: false, totp: false, backupCodes: false, phone: false } },
    { firstName: 'Issouf',  lastName: 'Bandé',     role: 'CLIENT',     region: 'Sahel',         phone: '+22670556677', mfaStatus: { email: true, passkey: false, totp: false, backupCodes: false, phone: false } },
    { firstName: 'Mariam',  lastName: 'Sawadogo',  role: 'CLIENT',     region: 'Centre',        phone: '+22670667788', mfaStatus: { email: true, passkey: true,  totp: false, backupCodes: false, phone: false } },
    { firstName: 'Abdou',   lastName: 'Diallo',    role: 'PRODUCTEUR', region: 'Centre',        phone: '+22670778899', mfaStatus: { email: true, passkey: false, totp: false, backupCodes: false, phone: false } },
    { firstName: 'Salif',   lastName: 'Koné',      role: 'PRODUCTEUR', region: 'Hauts-Bassins', phone: '+22670889900', mfaStatus: { email: true, passkey: true,  totp: true,  backupCodes: true, phone: false } },
    { firstName: 'Aminata', lastName: 'Yaméogo',   role: 'ADMIN',      region: 'Centre',        phone: '+22670990011', mfaStatus: { email: true, passkey: true,  totp: true,  backupCodes: true, phone: true  } },
    { firstName: 'Admin',   lastName: 'FASO',      role: 'ADMIN',      region: 'Centre',        phone: '+22671001122', mfaStatus: { email: true, passkey: true,  totp: true,  backupCodes: true, phone: false } },
  ];
  return sample.map((s, i) => ({
    id: 'u-' + (i + 1),
    email: `${s.firstName!.toLowerCase()}.${s.lastName!.toLowerCase().replace(/\s+/g, '')}@example.bf`,
    firstName: s.firstName!,
    lastName: s.lastName!,
    displayName: `${s.firstName} ${s.lastName}`,
    role: s.role!,
    phone: s.phone,
    region: s.region,
    isActive: true,
    mfaConfigured: Object.values(s.mfaStatus!).filter(Boolean).length >= 3,
    mfaStatus: s.mfaStatus!,
    createdAt: new Date(Date.now() - (i + 1) * 86400000 * 7).toISOString(),
    lastLoginAt: new Date(Date.now() - i * 3600000).toISOString(),
  }));
}
