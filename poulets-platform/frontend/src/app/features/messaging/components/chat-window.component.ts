// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, ElementRef, OnInit, ViewChild, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { FormsModule } from '@angular/forms';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { BreederAvatarComponent } from '@shared/components/breeder-avatar/breeder-avatar.component';

interface ChatMessage {
  id: string;
  from: 'me' | 'them';
  text: string;
  at: string;
  status?: 'sent' | 'delivered' | 'read';
}

@Component({
  selector: 'app-chat-window',
  standalone: true,
  imports: [CommonModule, FormsModule, RouterLink, DatePipe, MatIconModule, MatButtonModule, BreederAvatarComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="chat" data-testid="messaging-detail">
      <header class="bar">
        <a routerLink=".." class="back" aria-label="Retour" data-testid="messaging-action-back">
          <mat-icon>arrow_back</mat-icon>
        </a>
        <app-breeder-avatar size="md" [name]="peerName()" [verified]="true" />
        <div class="peer">
          <strong>{{ peerName() }}</strong>
          <span>{{ peerStatus() }}</span>
        </div>
        <button type="button" aria-label="Appeler">
          <mat-icon>phone</mat-icon>
        </button>
      </header>

      <div #scroller class="thread" role="log" aria-live="polite" data-testid="messaging-thread">
        @for (msg of messages(); track msg.id) {
          <div class="msg" [class.mine]="msg.from === 'me'"
               [attr.data-testid]="'messaging-thread-message-' + msg.id">
            <div class="bubble">{{ msg.text }}</div>
            <div class="meta">
              <span>{{ msg.at | date:'shortTime' }}</span>
              @if (msg.from === 'me' && msg.status) {
                <mat-icon
                  class="tick"
                  [class.read]="msg.status === 'read'"
                >
                  {{ msg.status === 'read' ? 'done_all' : msg.status === 'delivered' ? 'done_all' : 'done' }}
                </mat-icon>
              }
            </div>
          </div>
        }
      </div>

      <form class="composer" (submit)="send($event)" data-testid="messaging-form">
        <button type="button" aria-label="Joindre une image" data-testid="messaging-action-attach">
          <mat-icon>attach_file</mat-icon>
        </button>
        <input
          [(ngModel)]="draft"
          name="draft"
          type="text"
          placeholder="Écrire un message…"
          autocomplete="off"
          aria-label="Message"
          data-testid="messaging-form-message"
        >
        <button type="submit" [disabled]="!draft.trim()" aria-label="Envoyer"
                data-testid="messaging-form-submit">
          <mat-icon>send</mat-icon>
        </button>
      </form>
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); height: 100vh; }

    .chat {
      display: flex;
      flex-direction: column;
      height: 100%;
      max-width: 900px;
      margin: 0 auto;
      background: var(--faso-surface);
      border-left: 1px solid var(--faso-border);
      border-right: 1px solid var(--faso-border);
    }

    .bar {
      display: flex;
      align-items: center;
      gap: var(--faso-space-3);
      padding: var(--faso-space-3) var(--faso-space-4);
      border-bottom: 1px solid var(--faso-border);
      background: var(--faso-surface);
      position: sticky;
      top: 0;
      z-index: 1;
    }
    .back, .bar button {
      background: transparent;
      border: none;
      cursor: pointer;
      color: var(--faso-text);
      padding: 6px;
      border-radius: 50%;
      display: inline-flex;
    }
    .back:hover, .bar button:hover { background: var(--faso-surface-alt); }

    .peer { display: flex; flex-direction: column; flex: 1; min-width: 0; }
    .peer strong { font-size: var(--faso-text-base); }
    .peer span { color: var(--faso-success); font-size: var(--faso-text-xs); }

    .thread {
      flex: 1;
      padding: var(--faso-space-5) var(--faso-space-4);
      background:
        radial-gradient(ellipse at top, rgba(255, 204, 128, 0.08) 0%, transparent 70%),
        var(--faso-bg);
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-2);
      overflow-y: auto;
    }

    .msg {
      display: flex;
      flex-direction: column;
      align-items: flex-start;
      max-width: 75%;
    }
    .msg.mine { align-self: flex-end; align-items: flex-end; }

    .bubble {
      padding: 10px 14px;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: 18px;
      border-bottom-left-radius: 4px;
      line-height: var(--faso-leading-normal);
      color: var(--faso-text);
      box-shadow: var(--faso-shadow-xs);
      word-wrap: break-word;
    }
    .msg.mine .bubble {
      background: var(--faso-primary-600);
      color: #FFFFFF;
      border-color: var(--faso-primary-600);
      border-bottom-left-radius: 18px;
      border-bottom-right-radius: 4px;
    }

    .meta {
      display: inline-flex;
      align-items: center;
      gap: 2px;
      margin-top: 2px;
      font-size: var(--faso-text-xs);
      color: var(--faso-text-muted);
    }
    .tick { font-size: 14px; width: 14px; height: 14px; color: var(--faso-text-subtle); }
    .tick.read { color: var(--faso-info); }

    .composer {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: var(--faso-space-3);
      border-top: 1px solid var(--faso-border);
      background: var(--faso-surface);
    }
    .composer button {
      background: transparent;
      border: none;
      cursor: pointer;
      padding: 8px;
      border-radius: 50%;
      display: inline-flex;
      color: var(--faso-text-muted);
    }
    .composer button:hover { background: var(--faso-surface-alt); color: var(--faso-primary-700); }
    .composer button[type="submit"] { background: var(--faso-primary-600); color: #FFFFFF; }
    .composer button[type="submit"]:hover { background: var(--faso-primary-700); }
    .composer button[type="submit"][disabled] {
      background: var(--faso-border);
      cursor: not-allowed;
    }
    .composer input {
      flex: 1;
      padding: 10px 16px;
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-pill);
      font-family: inherit;
      font-size: var(--faso-text-base);
      color: var(--faso-text);
      background: var(--faso-surface-alt);
    }
    .composer input:focus {
      outline: none;
      border-color: var(--faso-primary-500);
      background: var(--faso-surface);
    }
  `],
})
export class ChatWindowComponent implements OnInit {
  @ViewChild('scroller') scroller!: ElementRef<HTMLDivElement>;
  private readonly route = inject(ActivatedRoute);

  readonly peerName = signal('Éleveur');
  readonly peerStatus = signal('En ligne · répond en moins de 2h');
  readonly messages = signal<ChatMessage[]>([]);
  draft = '';

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id') ?? '1';
    this.peerName.set(id === '2' ? 'Awa Sankara' : id === '3' ? 'Oumar Traoré' : 'Kassim Ouédraogo');
    this.messages.set([
      { id: 'm1', from: 'them', text: 'Bonjour ! Je peux vous livrer lundi si vous confirmez aujourd\'hui.', at: new Date(Date.now() - 3600000).toISOString() },
      { id: 'm2', from: 'me',   text: 'Super, quelle quantité est disponible ?', at: new Date(Date.now() - 3000000).toISOString(), status: 'read' },
      { id: 'm3', from: 'them', text: 'Jusqu\'à 50 poulets bicyclette, entre 1,8 et 2,2 kg.', at: new Date(Date.now() - 1800000).toISOString() },
      { id: 'm4', from: 'me',   text: 'Parfait, je prends 20 pour commencer. Vous livrez à Ouaga 2000 ?', at: new Date(Date.now() - 900000).toISOString(), status: 'delivered' },
    ]);
  }

  send(ev: Event): void {
    ev.preventDefault();
    const text = this.draft.trim();
    if (!text) return;

    this.messages.update(m => [
      ...m,
      { id: 'm' + Math.random().toString(36).slice(2, 8), from: 'me', text, at: new Date().toISOString(), status: 'sent' },
    ]);
    this.draft = '';

    // Simulated reply (replace with Apollo subscription on real BFF).
    setTimeout(() => {
      this.messages.update(m => [
        ...m,
        { id: 'm' + Math.random().toString(36).slice(2, 8), from: 'them', text: 'Bien reçu, je valide et je vous reviens vite.', at: new Date().toISOString() },
      ]);
      queueMicrotask(() => this.scrollToBottom());
    }, 1200);

    queueMicrotask(() => this.scrollToBottom());
  }

  private scrollToBottom() {
    const el = this.scroller?.nativeElement;
    if (el) el.scrollTop = el.scrollHeight;
  }
}
