import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatTableModule } from '@angular/material/table';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';

interface AdminUser {
  id: string;
  name: string;
  email: string;
  role: string;
  verified: boolean;
  registeredAt: string;
  lastActive: string;
}

@Component({
  selector: 'app-admin-users',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatTableModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule,
    MatFormFieldModule,
    MatInputModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
  ],
  template: `
    <div class="admin-users-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'admin.users.title' | translate }}</h1>
      </div>

      <!-- Role Summary -->
      <div class="role-chips">
        <mat-chip-listbox (change)="filterByRole($event.value)">
          <mat-chip-option value="all" selected>
            {{ 'admin.users.all' | translate }} ({{ users().length }})
          </mat-chip-option>
          <mat-chip-option value="eleveur">
            {{ 'admin.users.eleveurs' | translate }} ({{ countRole('eleveur') }})
          </mat-chip-option>
          <mat-chip-option value="client">
            {{ 'admin.users.clients' | translate }} ({{ countRole('client') }})
          </mat-chip-option>
          <mat-chip-option value="admin">
            {{ 'admin.users.admins' | translate }} ({{ countRole('admin') }})
          </mat-chip-option>
        </mat-chip-listbox>
      </div>

      <mat-card>
        <mat-card-content>
          <table mat-table [dataSource]="filteredUsers()" class="full-width-table">
            <ng-container matColumnDef="name">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.users.name' | translate }}</th>
              <td mat-cell *matCellDef="let u">{{ u.name }}</td>
            </ng-container>
            <ng-container matColumnDef="email">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.users.email' | translate }}</th>
              <td mat-cell *matCellDef="let u">{{ u.email }}</td>
            </ng-container>
            <ng-container matColumnDef="role">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.users.role' | translate }}</th>
              <td mat-cell *matCellDef="let u">
                <mat-chip class="role-chip">{{ u.role }}</mat-chip>
              </td>
            </ng-container>
            <ng-container matColumnDef="verified">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.users.verified' | translate }}</th>
              <td mat-cell *matCellDef="let u">
                @if (u.verified) {
                  <mat-icon class="verified-yes">verified</mat-icon>
                } @else {
                  <mat-icon class="verified-no">cancel</mat-icon>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="registeredAt">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.users.registered' | translate }}</th>
              <td mat-cell *matCellDef="let u">{{ u.registeredAt | date:'dd/MM/yyyy' }}</td>
            </ng-container>
            <ng-container matColumnDef="lastActive">
              <th mat-header-cell *matHeaderCellDef>{{ 'admin.users.last_active' | translate }}</th>
              <td mat-cell *matCellDef="let u">{{ u.lastActive | date:'dd/MM/yyyy' }}</td>
            </ng-container>
            <tr mat-header-row *matHeaderRowDef="displayedColumns"></tr>
            <tr mat-row *matRowDef="let row; columns: displayedColumns;"></tr>
          </table>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .admin-users-container {
      padding: 24px;
      max-width: 1200px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .role-chips { margin-bottom: 16px; }

    .full-width-table { width: 100%; }

    .role-chip { text-transform: capitalize; }

    .verified-yes { color: #4caf50; }
    .verified-no { color: #f44336; }
  `],
})
export class AdminUsersComponent implements OnInit {
  readonly users = signal<AdminUser[]>([]);
  readonly filteredUsers = signal<AdminUser[]>([]);
  readonly displayedColumns = ['name', 'email', 'role', 'verified', 'registeredAt', 'lastActive'];

  ngOnInit(): void {
    this.loadUsers();
  }

  countRole(role: string): number {
    return this.users().filter(u => u.role === role).length;
  }

  filterByRole(role: string): void {
    if (role === 'all') {
      this.filteredUsers.set(this.users());
    } else {
      this.filteredUsers.set(this.users().filter(u => u.role === role));
    }
  }

  private loadUsers(): void {
    const data: AdminUser[] = [
      { id: 'u1', name: 'Ouedraogo Moussa', email: 'moussa@example.com', role: 'eleveur', verified: true, registeredAt: '2025-06-01', lastActive: '2026-04-07' },
      { id: 'u2', name: 'Restaurant Le Sahel', email: 'sahel@example.com', role: 'client', verified: true, registeredAt: '2025-07-15', lastActive: '2026-04-07' },
      { id: 'u3', name: 'Kabore Amidou', email: 'amidou@example.com', role: 'eleveur', verified: true, registeredAt: '2025-08-01', lastActive: '2026-04-06' },
      { id: 'u4', name: 'Mme Traore', email: 'traore@example.com', role: 'client', verified: false, registeredAt: '2026-03-15', lastActive: '2026-04-07' },
      { id: 'u5', name: 'Admin Principal', email: 'admin@poulets.bf', role: 'admin', verified: true, registeredAt: '2025-01-01', lastActive: '2026-04-07' },
    ];
    this.users.set(data);
    this.filteredUsers.set(data);
  }
}
