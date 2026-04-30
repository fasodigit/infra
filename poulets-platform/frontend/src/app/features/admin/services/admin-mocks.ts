// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso
// Mock data sourced from Claude Design — admin-ui v1. Used until services are wired to BFF.

import type {
  AdminUser, AdminSession, TrustedDevice, AuditEntry,
  ServiceHealth, ChartPoint, SettingHistoryEntry,
} from '../models/admin.model';

export const MOCK_USERS: readonly AdminUser[] = [
  { id:'u1', firstName:'Aminata', lastName:'Ouédraogo', email:'aminata.ouedraogo@faso.bf', department:'Direction Générale', role:'SUPER-ADMIN', level:0, verified:true, mfa:{passkey:true,totp:true,backupCodes:9}, createdAt:'2024-03-12', lastActive:'il y a 2 min', status:'active', failedLogins:0, devices:3, avatar:'#1b5e20' },
  { id:'u2', firstName:'Souleymane', lastName:'Sawadogo', email:'s.sawadogo@faso.bf', department:'Sécurité Numérique', role:'SUPER-ADMIN', level:0, verified:true, mfa:{passkey:true,totp:true,backupCodes:10}, createdAt:'2024-04-02', lastActive:'il y a 14 min', status:'active', failedLogins:0, devices:2, avatar:'#b8860b' },
  { id:'u3', firstName:'Fatoumata', lastName:'Kaboré', email:'f.kabore@faso.bf', department:'État-Civil · Ouagadougou', role:'ADMIN', level:1, verified:true, mfa:{passkey:true,totp:false,backupCodes:8}, createdAt:'2024-06-18', lastActive:'il y a 1 h', status:'active', failedLogins:0, devices:2, avatar:'#2e7d32' },
  { id:'u4', firstName:'Ibrahim', lastName:'Compaoré', email:'ibrahim.compaore@sante.faso.bf', department:'Hôpital Yalgado', role:'ADMIN', level:1, verified:true, mfa:{passkey:false,totp:true,backupCodes:6}, createdAt:'2024-07-04', lastActive:'il y a 3 h', status:'active', failedLogins:1, devices:1, avatar:'#1565c0' },
  { id:'u5', firstName:'Mariam', lastName:'Traoré', email:'mariam.traore@education.faso.bf', department:'E-School · Bobo-Dioulasso', role:'ADMIN', level:1, verified:true, mfa:{passkey:true,totp:true,backupCodes:10}, createdAt:'2024-08-22', lastActive:'hier', status:'active', failedLogins:0, devices:2, avatar:'#c77700' },
  { id:'u6', firstName:'Ousmane', lastName:'Diallo', email:'o.diallo@faso.bf', department:'E-Ticket · Ouagadougou', role:'MANAGER', level:2, verified:true, mfa:{passkey:false,totp:true,backupCodes:8}, createdAt:'2024-09-10', lastActive:'il y a 26 min', status:'active', failedLogins:0, devices:1, avatar:'#7c4dff' },
  { id:'u7', firstName:'Salimata', lastName:'Ouattara', email:'salimata.ouattara@faso.bf', department:'SOGESY', role:'MANAGER', level:2, verified:true, mfa:{passkey:true,totp:false,backupCodes:10}, createdAt:'2024-10-15', lastActive:'il y a 47 min', status:'active', failedLogins:0, devices:1, avatar:'#00796b' },
  { id:'u8', firstName:'Boukary', lastName:'Zongo', email:'b.zongo@vouchers.faso.bf', department:'Vouchers', role:'MANAGER', level:2, verified:true, mfa:{passkey:false,totp:true,backupCodes:7}, createdAt:'2024-11-03', lastActive:'il y a 2 h', status:'active', failedLogins:2, devices:2, avatar:'#5d4037' },
  { id:'u9', firstName:'Awa', lastName:'Sangaré', email:'awa.sangare@altmission.faso.bf', department:'ALT-MISSION', role:'MANAGER', level:2, verified:false, mfa:{passkey:false,totp:false,backupCodes:0}, createdAt:'2026-04-12', lastActive:'jamais', status:'active', failedLogins:0, devices:0, avatar:'#d32f2f' },
  { id:'u10', firstName:'Drissa', lastName:'Yaméogo', email:'d.yameogo@fasokalan.faso.bf', department:'FASO-Kalan', role:'MANAGER', level:2, verified:true, mfa:{passkey:true,totp:true,backupCodes:9}, createdAt:'2024-12-01', lastActive:'il y a 4 h', status:'suspended', failedLogins:5, devices:1, avatar:'#455a64' },
];

export const MOCK_AUDIT: readonly AuditEntry[] = [
  { id:'a1', actor:'u1', action:'SETTINGS_UPDATED', target:'otp.lifetime_seconds', oldVal:300, newVal:600, time:'10:42:18', date:'30 avril 2026', traceId:'4f7c9e2a', ip:'196.28.111.42', desc:'Durée OTP modifiée de 5 min à 10 min' },
  { id:'a2', actor:'u3', action:'ROLE_GRANTED', target:'u9', desc:'Rôle MANAGER octroyé à Awa Sangaré (ALT-MISSION)', time:'10:38:04', date:'30 avril 2026', traceId:'8a1d3f47', ip:'196.28.111.18' },
  { id:'a3', actor:'u4', action:'BREAK_GLASS_ACTIVATED', target:'u4', desc:'Break-Glass activé · Justification: Incident SEV-1 base de données état-civil', time:'09:14:55', date:'30 avril 2026', traceId:'2c8e4b91', ip:'196.28.111.7', critical:true },
  { id:'a4', actor:'u2', action:'MFA_ENROLLED', target:'u3', desc:'PassKey YubiKey 5C enregistrée pour Fatoumata Kaboré', time:'08:52:11', date:'30 avril 2026', traceId:'f3a7c019', ip:'196.28.111.42' },
  { id:'a5', actor:'u1', action:'DEVICE_REVOKED', target:'u8', desc:'Appareil révoqué (Chrome 124 · Ubuntu 22.04)', time:'08:30:00', date:'30 avril 2026', traceId:'b91e2d4a', ip:'196.28.111.42' },
  { id:'a6', actor:'u3', action:'SESSION_REVOKED', target:'u10', desc:'Session forcée fermée pour Drissa Yaméogo', time:'08:11:42', date:'30 avril 2026', traceId:'7d52ab83', ip:'196.28.111.18' },
  { id:'a7', actor:'u2', action:'OTP_FAILED', target:'u10', desc:'OTP échoué (3/3) · Compte suspendu automatiquement', time:'07:44:22', date:'30 avril 2026', traceId:'c1e88f0d', ip:'41.207.99.4', critical:true },
  { id:'a8', actor:'u1', action:'USER_CREATED', target:'u9', desc:'Invitation envoyée à awa.sangare@altmission.faso.bf', time:'17:02:30', date:'29 avril 2026', traceId:'9b4d0c11', ip:'196.28.111.42' },
];

export const MOCK_SESSIONS: readonly AdminSession[] = [
  { id:'s1', user:'u1', token:'kratos_8f3a…2c91', created:'08:14', lastActive:"à l'instant", ip:'196.28.111.42', city:'Ouagadougou', device:'MacBook Pro · Safari 17', current:true },
  { id:'s2', user:'u3', token:'kratos_b71e…4d02', created:'09:02', lastActive:'il y a 1 h', ip:'196.28.111.18', city:'Ouagadougou', device:'Dell Latitude · Firefox 124' },
  { id:'s3', user:'u4', token:'kratos_2a90…77f5', created:'07:30', lastActive:'il y a 3 h', ip:'41.207.99.12', city:'Bobo-Dioulasso', device:'iPhone 15 · Safari Mobile' },
  { id:'s4', user:'u6', token:'kratos_5d11…9e6b', created:'10:08', lastActive:'il y a 26 min', ip:'196.28.111.91', city:'Ouagadougou', device:'Lenovo ThinkPad · Chrome 124' },
  { id:'s5', user:'u7', token:'kratos_c44a…b18d', created:'09:47', lastActive:'il y a 47 min', ip:'41.207.99.88', city:'Koudougou', device:'iPad Pro · Safari 17' },
  { id:'s6', user:'u8', token:'kratos_e07f…3a55', created:'08:55', lastActive:'il y a 2 h', ip:'196.28.111.55', city:'Ouagadougou', device:'HP EliteBook · Edge 124' },
];

export const MOCK_DEVICES: readonly TrustedDevice[] = [
  { id:'d1', user:'u1', fp:'a3f5c8d1e9b2', type:'YubiKey 5C',     ua:'Safari 17.4 · macOS Sonoma', ip:'196.28.111.42', city:'Ouagadougou',    created:'2024-03-12', lastUsed:"à l'instant", trustedUntil:'29 mai 2026' },
  { id:'d2', user:'u1', fp:'b71e9d2a4c08', type:'Touch ID',        ua:'Safari Mobile · iOS 17',     ip:'41.207.99.4',   city:'Ouagadougou',    created:'2024-08-04', lastUsed:'hier',         trustedUntil:'14 mai 2026' },
  { id:'d3', user:'u3', fp:'c8d402a17f55', type:'Windows Hello',   ua:'Edge 124 · Windows 11',      ip:'196.28.111.18', city:'Ouagadougou',    created:'2024-06-18', lastUsed:'il y a 1 h',   trustedUntil:'18 mai 2026' },
  { id:'d4', user:'u4', fp:'2c8e4b91d077', type:'WebAuthn',        ua:'Chrome 124 · Android 14',    ip:'41.207.99.12',  city:'Bobo-Dioulasso', created:'2024-07-04', lastUsed:'il y a 3 h',   trustedUntil:'04 mai 2026' },
  { id:'d5', user:'u6', fp:'5d11e07f3a55', type:'UA Hash',         ua:'Chrome 124 · Ubuntu 22.04',  ip:'196.28.111.91', city:'Ouagadougou',    created:'2024-09-10', lastUsed:'il y a 26 min',trustedUntil:'10 mai 2026' },
  { id:'d6', user:'u8', fp:'e07f9c2b18d4', type:'YubiKey 5 NFC',   ua:'Firefox 124 · Fedora 39',    ip:'196.28.111.55', city:'Ouagadougou',    created:'2024-11-03', lastUsed:'il y a 2 h',   trustedUntil:'03 juin 2026' },
];

export const MOCK_CHART: readonly ChartPoint[] = [
  { d:'Lun', otp:284, sessions:42 }, { d:'Mar', otp:312, sessions:48 },
  { d:'Mer', otp:298, sessions:51 }, { d:'Jeu', otp:421, sessions:58 },
  { d:'Ven', otp:389, sessions:54 }, { d:'Sam', otp:142, sessions:22 },
  { d:'Dim', otp:118, sessions:18 },
];

export const MOCK_SERVICES: readonly ServiceHealth[] = [
  { name:'ARMAGEDDON', port:':8080',  status:'ok',   meta:'Pingora · 4ms p99' },
  { name:'auth-ms',    port:':8801',  status:'ok',   meta:'Java 21 · 18ms p99' },
  { name:'KAYA',       port:':6380',  status:'ok',   meta:'RESP3 · 0.6ms' },
  { name:'Kratos',     port:':4433',  status:'ok',   meta:'session 8h Lax' },
  { name:'Keto',       port:':4466',  status:'warn', meta:'circuit-breaker · 3 retries' },
  { name:'PostgreSQL', port:':5432',  status:'ok',   meta:'17.2 · 24 conn' },
  { name:'Redpanda',   port:':19092', status:'ok',   meta:'3 brokers · 12 topics' },
  { name:'Mailpit',    port:':1025',  status:'ok',   meta:'SMTP dev' },
];

export const MOCK_SETTINGS_HISTORY: readonly SettingHistoryEntry[] = [
  { v:4, when:'30 avril 2026 · 10:42', who:'Aminata Ouédraogo',   oldV:300, newV:600, motif:'Plaintes utilisateurs · délai trop court en zone réseau lente', trace:'4f7c9e2a' },
  { v:3, when:'12 mars 2026 · 14:22',  who:'Souleymane Sawadogo', oldV:240, newV:300, motif:'Alignement avec recommandation ANSSI-BF', trace:'a91c0d34' },
  { v:2, when:'04 février 2026 · 09:15', who:'Aminata Ouédraogo', oldV:180, newV:240, motif:'Retours pilote État-Civil', trace:'1e7b9f2c' },
  { v:1, when:'18 décembre 2025 · 16:48', who:'system · seed',    oldV:null, newV:180, motif:'Valeur initiale (migration V5)', trace:'init' },
];
