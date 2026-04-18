import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./conversations-list.component').then(m => m.ConversationsListComponent),
    title: 'Messagerie - Poulets Platform',
  },
  {
    path: ':conversationId',
    loadComponent: () =>
      import('./components/chat-window.component').then(m => m.ChatWindowComponent),
    title: 'Conversation - Poulets BF',
  },
  {
    path: ':conversationId/legacy',
    loadComponent: () =>
      import('./chat.component').then(m => m.ChatComponent),
    title: 'Conversation (legacy) - Poulets Platform',
  },
] as Routes;
