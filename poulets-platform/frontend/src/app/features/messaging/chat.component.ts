import { Component, OnInit, signal, ViewChild, ElementRef } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { FormsModule } from '@angular/forms';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatMenuModule } from '@angular/material/menu';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';
import { FcfaCurrencyPipe } from '@shared/pipes/currency.pipe';

type MessageType = 'text' | 'price_proposal' | 'price_accepted' | 'counter_offer';

interface ChatMessage {
  id: string;
  senderId: string;
  content: string;
  type: MessageType;
  priceValue?: number;
  timestamp: string;
  isMine: boolean;
}

@Component({
  selector: 'app-chat',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    FormsModule,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatInputModule,
    MatFormFieldModule,
    MatMenuModule,
    MatDividerModule,
    TranslateModule,
    FcfaCurrencyPipe,
    DatePipe,
  ],
  template: `
    <div class="chat-container">
      <!-- Chat Header -->
      <div class="chat-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <div class="header-info">
          <div class="header-avatar">{{ getInitials(participantName()) }}</div>
          <div>
            <span class="header-name">{{ participantName() }}</span>
            <span class="header-role">{{ participantRole() }}</span>
          </div>
        </div>
        <span class="spacer"></span>
        <button mat-icon-button [matMenuTriggerFor]="actionMenu">
          <mat-icon>more_vert</mat-icon>
        </button>
        <mat-menu #actionMenu="matMenu">
          <button mat-menu-item (click)="openNegotiation()">
            <mat-icon>handshake</mat-icon>
            {{ 'messaging.chat.negotiate' | translate }}
          </button>
        </mat-menu>
      </div>

      <!-- Messages Area -->
      <div class="messages-area" #messagesArea>
        @for (msg of messages(); track msg.id) {
          <div class="message-wrapper" [class.mine]="msg.isMine">
            @if (msg.type === 'text') {
              <div class="message-bubble" [class.sent]="msg.isMine" [class.received]="!msg.isMine">
                <p class="message-text">{{ msg.content }}</p>
                <span class="message-time">{{ msg.timestamp | date:'HH:mm' }}</span>
              </div>
            } @else if (msg.type === 'price_proposal') {
              <div class="negotiation-bubble proposal">
                <mat-icon>local_offer</mat-icon>
                <div class="negotiation-content">
                  <span class="negotiation-label">{{ 'messaging.chat.price_proposal' | translate }}</span>
                  <span class="negotiation-price">{{ msg.priceValue | fcfa }}</span>
                  <span class="negotiation-text">{{ msg.content }}</span>
                </div>
                @if (!msg.isMine) {
                  <div class="negotiation-actions">
                    <button mat-raised-button color="primary" (click)="acceptPrice(msg.priceValue!)">
                      {{ 'messaging.chat.accept' | translate }}
                    </button>
                    <button mat-stroked-button (click)="showCounterOffer = true">
                      {{ 'messaging.chat.counter' | translate }}
                    </button>
                  </div>
                }
              </div>
            } @else if (msg.type === 'price_accepted') {
              <div class="negotiation-bubble accepted">
                <mat-icon>check_circle</mat-icon>
                <div class="negotiation-content">
                  <span class="negotiation-label">{{ 'messaging.chat.price_accepted' | translate }}</span>
                  <span class="negotiation-price">{{ msg.priceValue | fcfa }}</span>
                </div>
              </div>
            } @else if (msg.type === 'counter_offer') {
              <div class="negotiation-bubble counter" [class.mine]="msg.isMine">
                <mat-icon>swap_horiz</mat-icon>
                <div class="negotiation-content">
                  <span class="negotiation-label">{{ 'messaging.chat.counter_offer' | translate }}</span>
                  <span class="negotiation-price">{{ msg.priceValue | fcfa }}</span>
                  <span class="negotiation-text">{{ msg.content }}</span>
                </div>
              </div>
            }
          </div>
        }
      </div>

      <!-- Counter Offer Dialog -->
      @if (showCounterOffer) {
        <div class="counter-offer-bar">
          <mat-form-field appearance="outline" class="counter-field">
            <mat-label>{{ 'messaging.chat.your_price' | translate }} (FCFA)</mat-label>
            <input matInput type="number" [(ngModel)]="counterPrice">
          </mat-form-field>
          <button mat-raised-button color="primary" (click)="sendCounterOffer()"
                  [disabled]="!counterPrice">
            {{ 'messaging.chat.send_counter' | translate }}
          </button>
          <button mat-icon-button (click)="showCounterOffer = false">
            <mat-icon>close</mat-icon>
          </button>
        </div>
      }

      <!-- Input Area -->
      <div class="input-area">
        <button mat-icon-button [matMenuTriggerFor]="negotiateMenu" color="primary">
          <mat-icon>add_circle</mat-icon>
        </button>
        <mat-menu #negotiateMenu="matMenu">
          <button mat-menu-item (click)="showPriceProposal = true">
            <mat-icon>local_offer</mat-icon>
            {{ 'messaging.chat.propose_price' | translate }}
          </button>
        </mat-menu>

        @if (showPriceProposal) {
          <mat-form-field appearance="outline" class="price-field">
            <mat-label>{{ 'messaging.chat.price' | translate }} (FCFA)</mat-label>
            <input matInput type="number" [(ngModel)]="proposedPrice">
          </mat-form-field>
          <button mat-raised-button color="primary" (click)="sendPriceProposal()"
                  [disabled]="!proposedPrice">
            {{ 'messaging.chat.propose' | translate }}
          </button>
          <button mat-icon-button (click)="showPriceProposal = false">
            <mat-icon>close</mat-icon>
          </button>
        } @else {
          <mat-form-field appearance="outline" class="message-field">
            <input matInput [(ngModel)]="newMessage"
                   (keyup.enter)="sendMessage()"
                   [placeholder]="'messaging.chat.placeholder' | translate">
          </mat-form-field>
          <button mat-icon-button color="primary" (click)="sendMessage()" [disabled]="!newMessage">
            <mat-icon>send</mat-icon>
          </button>
        }
      </div>
    </div>
  `,
  styles: [`
    .chat-container {
      display: flex;
      flex-direction: column;
      height: calc(100vh - 64px);
      max-width: 800px;
      margin: 0 auto;
    }

    .chat-header {
      display: flex;
      align-items: center;
      gap: 12px;
      padding: 12px 16px;
      background: white;
      border-bottom: 1px solid #e0e0e0;

      .spacer { flex: 1; }
    }

    .header-info {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .header-avatar {
      width: 40px;
      height: 40px;
      border-radius: 50%;
      background: var(--faso-primary, #2e7d32);
      color: white;
      display: flex;
      align-items: center;
      justify-content: center;
      font-weight: 600;
    }

    .header-name { font-weight: 600; display: block; }
    .header-role { font-size: 0.8rem; color: #666; display: block; }

    .messages-area {
      flex: 1;
      overflow-y: auto;
      padding: 16px;
      display: flex;
      flex-direction: column;
      gap: 8px;
      background: #f5f5f5;
    }

    .message-wrapper {
      display: flex;

      &.mine { justify-content: flex-end; }
    }

    .message-bubble {
      max-width: 70%;
      padding: 10px 14px;
      border-radius: 16px;
      position: relative;

      &.sent {
        background: var(--faso-primary, #2e7d32);
        color: white;
        border-bottom-right-radius: 4px;
      }

      &.received {
        background: white;
        border-bottom-left-radius: 4px;
      }

      .message-text {
        margin: 0;
        font-size: 0.9rem;
        line-height: 1.4;
      }

      .message-time {
        font-size: 0.65rem;
        opacity: 0.7;
        display: block;
        text-align: right;
        margin-top: 4px;
      }
    }

    .negotiation-bubble {
      max-width: 80%;
      padding: 14px 16px;
      border-radius: 12px;
      display: flex;
      gap: 12px;
      align-items: flex-start;

      &.proposal {
        background: #fff3e0;
        border: 1px solid #ff9800;
      }

      &.accepted {
        background: #e8f5e9;
        border: 1px solid #4caf50;
      }

      &.counter {
        background: #e3f2fd;
        border: 1px solid #2196f3;
      }

      mat-icon {
        min-width: 24px;
        margin-top: 2px;
      }

      &.proposal mat-icon { color: #ff9800; }
      &.accepted mat-icon { color: #4caf50; }
      &.counter mat-icon { color: #2196f3; }
    }

    .negotiation-content {
      display: flex;
      flex-direction: column;
      gap: 4px;

      .negotiation-label { font-size: 0.75rem; font-weight: 600; text-transform: uppercase; }
      .negotiation-price { font-size: 1.2rem; font-weight: 700; }
      .negotiation-text { font-size: 0.85rem; color: #666; }
    }

    .negotiation-actions {
      display: flex;
      gap: 8px;
      margin-top: 8px;
    }

    .counter-offer-bar {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 8px 16px;
      background: #e3f2fd;
      border-top: 1px solid #bbdefb;

      .counter-field { flex: 1; margin: 0; }
    }

    .input-area {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 8px 16px;
      background: white;
      border-top: 1px solid #e0e0e0;

      .message-field, .price-field {
        flex: 1;
        margin: 0;
      }
    }
  `],
})
export class ChatComponent implements OnInit {
  @ViewChild('messagesArea') messagesArea!: ElementRef;

  readonly participantName = signal('Restaurant Le Sahel');
  readonly participantRole = signal('Client');
  readonly messages = signal<ChatMessage[]>([]);

  newMessage = '';
  proposedPrice = 0;
  counterPrice = 0;
  showPriceProposal = false;
  showCounterOffer = false;

  private nextId = 100;

  constructor(private readonly route: ActivatedRoute) {}

  ngOnInit(): void {
    this.loadMessages();
  }

  getInitials(name: string): string {
    return name.split(' ').map(n => n[0]).join('').substring(0, 2).toUpperCase();
  }

  sendMessage(): void {
    if (!this.newMessage.trim()) return;
    this.addMessage({
      id: `msg-${this.nextId++}`,
      senderId: 'me',
      content: this.newMessage,
      type: 'text',
      timestamp: new Date().toISOString(),
      isMine: true,
    });
    this.newMessage = '';
  }

  sendPriceProposal(): void {
    if (!this.proposedPrice) return;
    this.addMessage({
      id: `msg-${this.nextId++}`,
      senderId: 'me',
      content: 'Prix propose par unite',
      type: 'price_proposal',
      priceValue: this.proposedPrice,
      timestamp: new Date().toISOString(),
      isMine: true,
    });
    this.proposedPrice = 0;
    this.showPriceProposal = false;
  }

  sendCounterOffer(): void {
    if (!this.counterPrice) return;
    this.addMessage({
      id: `msg-${this.nextId++}`,
      senderId: 'me',
      content: 'Contre-proposition',
      type: 'counter_offer',
      priceValue: this.counterPrice,
      timestamp: new Date().toISOString(),
      isMine: true,
    });
    this.counterPrice = 0;
    this.showCounterOffer = false;
  }

  acceptPrice(price: number): void {
    this.addMessage({
      id: `msg-${this.nextId++}`,
      senderId: 'me',
      content: '',
      type: 'price_accepted',
      priceValue: price,
      timestamp: new Date().toISOString(),
      isMine: true,
    });
  }

  openNegotiation(): void {
    this.showPriceProposal = true;
  }

  private addMessage(msg: ChatMessage): void {
    this.messages.update(msgs => [...msgs, msg]);
    setTimeout(() => this.scrollToBottom(), 50);
  }

  private scrollToBottom(): void {
    if (this.messagesArea) {
      const el = this.messagesArea.nativeElement;
      el.scrollTop = el.scrollHeight;
    }
  }

  private loadMessages(): void {
    this.messages.set([
      {
        id: 'msg-1', senderId: 'other', content: 'Bonjour, j\'ai vu votre annonce pour les poulets bicyclette.',
        type: 'text', timestamp: '2026-04-06T10:00:00', isMine: false,
      },
      {
        id: 'msg-2', senderId: 'me', content: 'Bonjour ! Oui, j\'ai 50 tetes disponibles, poids moyen 2.1 kg.',
        type: 'text', timestamp: '2026-04-06T10:05:00', isMine: true,
      },
      {
        id: 'msg-3', senderId: 'other', content: 'Quel est votre prix par tete ?',
        type: 'text', timestamp: '2026-04-06T10:08:00', isMine: false,
      },
      {
        id: 'msg-4', senderId: 'me', content: 'Prix propose par tete',
        type: 'price_proposal', priceValue: 3500,
        timestamp: '2026-04-06T10:12:00', isMine: true,
      },
      {
        id: 'msg-5', senderId: 'other', content: 'Contre-proposition pour 50 tetes',
        type: 'counter_offer', priceValue: 3200,
        timestamp: '2026-04-06T10:20:00', isMine: false,
      },
      {
        id: 'msg-6', senderId: 'me',
        content: 'Pour 50 tetes, je peux faire 3300 FCFA. C\'est mon meilleur prix.',
        type: 'text', timestamp: '2026-04-06T10:25:00', isMine: true,
      },
      {
        id: 'msg-7', senderId: 'other',
        content: 'D\'accord, 3300 FCFA par tete. Est-ce que les poulets seront prets pour jeudi ?',
        type: 'text', timestamp: '2026-04-07T09:30:00', isMine: false,
      },
    ]);
  }
}
