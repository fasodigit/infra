import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./conversations-list.component').then(m => m.ConversationsListComponent),
    title: 'Messagerie - Poulets Platform',
  },
  // Realtime chat stub (F4) — sous-chemin explicite pour éviter de casser
  // l'existant `:conversationId` qui pointe vers ChatWindowComponent.
  {
    path: 'chat/:conversationId',
    loadComponent: () =>
      import('./components/chat-realtime.component').then(m => m.ChatRealtimeComponent),
    title: 'Chat temps réel - Poulets Platform',
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
