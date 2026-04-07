import { Component, inject, ChangeDetectionStrategy } from '@angular/core';
import { TranslateModule } from '@ngx-translate/core';
import { MatCardModule } from '@angular/material/card';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { AuthService } from '@core/services/auth.service';

@Component({
  selector: 'app-profile-home',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [TranslateModule, MatCardModule, MatIconModule, MatButtonModule],
  template: `
    <div class="page-container">
      <h1>{{ 'profile.title' | translate }}</h1>
      <mat-card>
        <mat-card-header>
          <mat-icon mat-card-avatar>account_circle</mat-icon>
          <mat-card-title>{{ auth.currentUser()?.nom }}</mat-card-title>
          <mat-card-subtitle>{{ auth.currentUser()?.email }}</mat-card-subtitle>
        </mat-card-header>
        <mat-card-content>
          <p>{{ 'profile.personal_info' | translate }}</p>
        </mat-card-content>
        <mat-card-actions align="end">
          <button mat-raised-button color="primary">
            <mat-icon>edit</mat-icon>
            {{ 'profile.edit' | translate }}
          </button>
        </mat-card-actions>
      </mat-card>
    </div>
  `,
  styles: [`.page-container { padding: 24px; max-width: 1200px; margin: 0 auto; }`],
})
export class ProfileHomeComponent {
  readonly auth = inject(AuthService);
}
