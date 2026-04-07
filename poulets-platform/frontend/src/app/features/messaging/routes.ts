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
      import('./chat.component').then(m => m.ChatComponent),
    title: 'Conversation - Poulets Platform',
  },
] as Routes;
