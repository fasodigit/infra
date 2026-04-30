import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatListModule } from '@angular/material/list';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatBadgeModule } from '@angular/material/badge';
import { MatInputModule } from '@angular/material/input';
import { MatFormFieldModule } from '@angular/material/form-field';
import { TranslateModule } from '@ngx-translate/core';

interface Conversation {
  id: string;
  participantName: string;
  participantAvatar?: string;
  participantRole: string;
  lastMessage: string;
  lastMessageDate: string;
  unreadCount: number;
}

@Component({
  selector: 'app-conversations-list',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatListModule,
    MatButtonModule,
    MatIconModule,
    MatBadgeModule,
    MatInputModule,
    MatFormFieldModule,
    TranslateModule,
    DatePipe,
  ],
  template: `
    <div class="conversations-container" data-testid="messaging-page">
      <div class="page-header">
        <h1>Messagerie</h1>
      </div>

      <!-- Search -->
      <mat-form-field appearance="outline" class="search-field">
        <mat-label>{{ 'messaging.list.search' | translate }}</mat-label>
        <mat-icon matPrefix>search</mat-icon>
        <input matInput (input)="onSearch($event)" data-testid="messaging-search-input">
      </mat-form-field>

      <mat-card class="conversations-card">
        @if (filteredConversations().length > 0) {
          <mat-nav-list data-testid="messaging-list">
            @for (conv of filteredConversations(); track conv.id) {
              <a mat-list-item [routerLink]="[conv.id]" class="conversation-item"
                 [attr.data-testid]="'messaging-list-item-' + conv.id">
                <div class="conversation-content">
                  <div class="avatar-section">
                    <div class="avatar" [class.has-unread]="conv.unreadCount > 0">
                      {{ getInitials(conv.participantName) }}
                    </div>
                    @if (conv.unreadCount > 0) {
                      <span class="unread-badge">{{ conv.unreadCount }}</span>
                    }
                  </div>
                  <div class="message-section">
                    <div class="message-header">
                      <span class="participant-name" [class.unread]="conv.unreadCount > 0">
                        {{ conv.participantName }}
                      </span>
                      <span class="message-time">{{ formatTime(conv.lastMessageDate) }}</span>
                    </div>
                    <div class="message-preview">
                      <span class="participant-role">{{ conv.participantRole }}</span>
                      <span class="last-message" [class.unread]="conv.unreadCount > 0">
                        {{ conv.lastMessage }}
                      </span>
                    </div>
                  </div>
                </div>
              </a>
            }
          </mat-nav-list>
        } @else {
          <div class="empty-state" data-testid="messaging-empty">
            <mat-icon>chat</mat-icon>
            <p>{{ 'messaging.list.empty' | translate }}</p>
          </div>
        }
      </mat-card>
    </div>
  `,
  styles: [`
    .conversations-container {
      padding: 24px;
      max-width: 700px;
      margin: 0 auto;
    }

    .page-header {
      margin-bottom: 16px;
      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .search-field { width: 100%; margin-bottom: 8px; }

    .conversations-card { padding: 0; }

    .conversation-item {
      height: auto !important;
      padding: 12px 16px !important;
      border-bottom: 1px solid #f0f0f0;
    }

    .conversation-content {
      display: flex;
      gap: 12px;
      width: 100%;
      align-items: center;
    }

    .avatar-section { position: relative; }

    .avatar {
      width: 48px;
      height: 48px;
      border-radius: 50%;
      background: #e0e0e0;
      display: flex;
      align-items: center;
      justify-content: center;
      font-weight: 600;
      font-size: 1rem;
      color: #666;

      &.has-unread {
        background: var(--faso-primary, #2e7d32);
        color: white;
      }
    }

    .unread-badge {
      position: absolute;
      top: -4px;
      right: -4px;
      background: #f44336;
      color: white;
      border-radius: 50%;
      width: 20px;
      height: 20px;
      font-size: 0.7rem;
      display: flex;
      align-items: center;
      justify-content: center;
      font-weight: 600;
    }

    .message-section {
      flex: 1;
      min-width: 0;
    }

    .message-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 4px;

      .participant-name {
        font-weight: 500;
        font-size: 0.95rem;

        &.unread { font-weight: 700; }
      }

      .message-time {
        font-size: 0.75rem;
        color: #999;
        white-space: nowrap;
      }
    }

    .message-preview {
      display: flex;
      flex-direction: column;
      gap: 2px;

      .participant-role {
        font-size: 0.75rem;
        color: var(--faso-primary, #2e7d32);
      }

      .last-message {
        font-size: 0.85rem;
        color: #888;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;

        &.unread { color: #333; font-weight: 500; }
      }
    }

    .empty-state {
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 48px 24px;
      color: #999;

      mat-icon { font-size: 48px; width: 48px; height: 48px; margin-bottom: 16px; }
    }
  `],
})
export class ConversationsListComponent implements OnInit {
  readonly conversations = signal<Conversation[]>([]);
  readonly filteredConversations = signal<Conversation[]>([]);

  ngOnInit(): void {
    this.loadConversations();
  }

  onSearch(event: Event): void {
    const query = (event.target as HTMLInputElement).value.toLowerCase();
    if (!query) {
      this.filteredConversations.set(this.conversations());
    } else {
      this.filteredConversations.set(
        this.conversations().filter(c =>
          c.participantName.toLowerCase().includes(query) ||
          c.lastMessage.toLowerCase().includes(query)
        )
      );
    }
  }

  getInitials(name: string): string {
    return name.split(' ').map(n => n[0]).join('').substring(0, 2).toUpperCase();
  }

  formatTime(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffHours = diffMs / (1000 * 60 * 60);

    if (diffHours < 1) return `${Math.floor(diffMs / 60000)}min`;
    if (diffHours < 24) return `${Math.floor(diffHours)}h`;
    return date.toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit' });
  }

  private loadConversations(): void {
    const data: Conversation[] = [
      {
        id: 'conv-1', participantName: 'Restaurant Le Sahel', participantRole: 'Client',
        lastMessage: 'Bonjour, est-ce que les 50 poulets seront prets pour jeudi ?',
        lastMessageDate: '2026-04-07T09:30:00', unreadCount: 2,
      },
      {
        id: 'conv-2', participantName: 'Mme Traore', participantRole: 'Client',
        lastMessage: 'Je propose 3800 FCFA par tete pour les pintades',
        lastMessageDate: '2026-04-07T08:15:00', unreadCount: 1,
      },
      {
        id: 'conv-3', participantName: 'Ibrahim Kabore', participantRole: 'Livreur',
        lastMessage: 'La livraison est confirmee pour demain matin',
        lastMessageDate: '2026-04-06T17:00:00', unreadCount: 0,
      },
      {
        id: 'conv-4', participantName: 'Dr. Sawadogo', participantRole: 'Veterinaire',
        lastMessage: 'Les resultats du controle sont bons. RAS.',
        lastMessageDate: '2026-04-05T14:30:00', unreadCount: 0,
      },
      {
        id: 'conv-5', participantName: 'Hotel Splendide', participantRole: 'Client',
        lastMessage: 'Offre acceptee ! On confirme la commande de 100 poulets.',
        lastMessageDate: '2026-04-04T11:00:00', unreadCount: 0,
      },
    ];
    this.conversations.set(data);
    this.filteredConversations.set(data);
  }
}
