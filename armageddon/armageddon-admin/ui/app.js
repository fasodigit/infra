// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
// ARMAGEDDON Admin UI — vanilla JS, no external CDN deps.

'use strict';

// -- constants --
const API_BASE = '/admin/api';
const POLL_MS  = 2000;

// -- state --
let activeTab  = 'overview';
let pollHandle = null;
let startTime  = Date.now();

// -- utils --

function token() {
  return document.getElementById('token-input').value.trim();
}

function headers() {
  const h = { 'Content-Type': 'application/json' };
  const t = token();
  if (t) h['X-Admin-Token'] = t;
  return h;
}

async function apiFetch(path, opts) {
  const res = await fetch(API_BASE + path, {
    headers: headers(),
    ...opts,
  });
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(res.status + ' ' + res.statusText + (body ? ': ' + body : ''));
  }
  return res.json();
}

function toast(msg, kind) {
  const area = document.getElementById('toast-area');
  const el = document.createElement('div');
  el.className = 'toast ' + (kind || '');
  el.textContent = msg;
  area.appendChild(el);
  setTimeout(() => el.remove(), 3500);
}

function badge(text, kind) {
  return '<span class="badge badge-' + kind + '">' + esc(text) + '</span>';
}

function esc(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function fmt(n) {
  if (n === null || n === undefined) return '—';
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
  if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
  return String(n);
}

function fmtMs(ms) {
  if (ms < 1000) return ms + 'ms';
  if (ms < 60000) return (ms / 1000).toFixed(1) + 's';
  const m = Math.floor(ms / 60000);
  const s = Math.floor((ms % 60000) / 1000);
  return m + 'm ' + s + 's';
}

function fmtUptime(startTs) {
  return fmtMs(Date.now() - startTs);
}

// -- tab navigation --

function showTab(name) {
  activeTab = name;
  document.querySelectorAll('.tab').forEach(el => {
    el.classList.toggle('active', el.dataset.tab === name);
  });
  document.querySelectorAll('.page').forEach(el => {
    el.classList.toggle('active', el.id === 'page-' + name);
  });
  refreshPage();
}

document.querySelectorAll('.tab').forEach(el => {
  el.addEventListener('click', () => showTab(el.dataset.tab));
});

// -- overview page --

async function refreshOverview() {
  const statsEl = document.getElementById('overview-stats');
  try {
    const data = await apiFetch('/stats');
    const counters = (data.counters || []);

    function pick(name) {
      const c = counters.find(c => c.name === name);
      return c ? c.value : 0;
    }

    const totalReq   = pick('armageddon_requests_total');
    const totalErr   = pick('armageddon_errors_total');
    const p99        = pick('armageddon_request_duration_p99_ms');
    const uptime     = fmtUptime(startTime);

    // rough RPS from total requests over poll interval (approximation)
    const prevTotal  = statsEl.dataset.prevTotal || 0;
    const prevTs     = statsEl.dataset.prevTs    || Date.now();
    const elapsed    = (Date.now() - Number(prevTs)) / 1000;
    const rps        = elapsed > 0 ? ((totalReq - prevTotal) / elapsed).toFixed(1) : '—';
    statsEl.dataset.prevTotal = totalReq;
    statsEl.dataset.prevTs    = Date.now();

    const errRate = totalReq > 0 ? ((totalErr / totalReq) * 100).toFixed(2) + '%' : '0%';

    document.getElementById('stat-uptime').textContent   = uptime;
    document.getElementById('stat-total-req').textContent = fmt(totalReq);
    document.getElementById('stat-rps').textContent       = rps;
    document.getElementById('stat-errrate').textContent   = errRate;
    document.getElementById('stat-p99').textContent       = p99 > 0 ? p99.toFixed(1) + 'ms' : '—';

    const errEl = document.getElementById('stat-errrate');
    errEl.className = 'stat-value' + (parseFloat(errRate) > 5 ? ' red' : ' green');

  } catch (e) {
    toast('Overview fetch failed: ' + e.message, 'err');
  }
}

// -- clusters page --

async function refreshClusters() {
  const tbody = document.getElementById('clusters-tbody');
  try {
    const data = await apiFetch('/clusters');
    const clusters = data.clusters || [];
    if (clusters.length === 0) {
      tbody.innerHTML = '<tr><td colspan="6" style="color:var(--text-muted);text-align:center">No clusters configured</td></tr>';
      return;
    }
    tbody.innerHTML = clusters.map(c => {
      const total   = c.endpoints.length;
      const healthy = c.endpoints.filter(e => e.healthy).length;
      const ep      = c.endpoints.map(e =>
        '<span title="' + esc(e.address + ':' + e.port) + '" style="color:' +
        (e.healthy ? 'var(--green-light)' : 'var(--red-bf)') + '">' +
        esc(e.address) + ':' + e.port + '</span>'
      ).join(', ');
      const drain  = c.draining ? badge('DRAINING', 'drain') : badge('ACTIVE', 'ok');
      const health = healthy === total ? badge(healthy + '/' + total + ' healthy', 'ok')
                   : healthy === 0     ? badge('0/' + total + ' DOWN', 'error')
                   :                    badge(healthy + '/' + total, 'warn');
      return '<tr>'
        + '<td>' + esc(c.name) + '</td>'
        + '<td>' + ep + '</td>'
        + '<td>' + health + '</td>'
        + '<td>' + drain + '</td>'
        + '<td>' + c.circuit_breaker.max_connections + '</td>'
        + '<td>'
        + '<button class="btn danger" onclick="drainCluster(\'' + esc(c.name) + '\')" '
        + (c.draining ? 'disabled' : '') + '>Drain</button>'
        + '</td>'
        + '</tr>';
    }).join('');
  } catch (e) {
    tbody.innerHTML = '<tr><td colspan="6" style="color:var(--red-bf)">' + esc(e.message) + '</td></tr>';
    toast('Clusters fetch failed: ' + e.message, 'err');
  }
}

async function drainCluster(name) {
  try {
    await apiFetch('/clusters/' + encodeURIComponent(name) + '/drain', { method: 'POST' });
    toast('Cluster "' + name + '" drain initiated', 'ok');
    refreshClusters();
  } catch (e) {
    toast('Drain failed: ' + e.message, 'err');
  }
}

// -- listeners page --

async function refreshListeners() {
  const tbody = document.getElementById('listeners-tbody');
  try {
    const data = await apiFetch('/listeners');
    const listeners = data.listeners || [];
    if (listeners.length === 0) {
      tbody.innerHTML = '<tr><td colspan="5" style="color:var(--text-muted);text-align:center">No listeners configured</td></tr>';
      return;
    }
    tbody.innerHTML = listeners.map(l => {
      const tls = l.tls_enabled ? badge('TLS', 'ok') : badge('PLAIN', 'warn');
      const proto = typeof l.protocol === 'string' ? l.protocol
                  : (l.protocol && l.protocol.type ? l.protocol.type : JSON.stringify(l.protocol));
      return '<tr>'
        + '<td>' + esc(l.name) + '</td>'
        + '<td>' + esc(l.address) + '</td>'
        + '<td style="color:var(--amber)">' + esc(String(l.port)) + '</td>'
        + '<td>' + esc(proto) + '</td>'
        + '<td>' + tls + (l.tls_min_version ? ' <span style="color:var(--text-muted)">' + esc(l.tls_min_version) + '</span>' : '') + '</td>'
        + '</tr>';
    }).join('');
  } catch (e) {
    tbody.innerHTML = '<tr><td colspan="5" style="color:var(--red-bf)">' + esc(e.message) + '</td></tr>';
    toast('Listeners fetch failed: ' + e.message, 'err');
  }
}

// -- stats page --

async function refreshStats() {
  const container = document.getElementById('stats-container');
  try {
    const data = await apiFetch('/stats');
    const counters = data.counters || [];
    if (counters.length === 0) {
      container.innerHTML = '<p style="color:var(--text-muted)">No metrics registered.</p>';
      return;
    }
    const rows = counters.map(c => {
      const labelStr = Object.entries(c.labels || {})
        .map(([k, v]) => '<span style="color:var(--text-muted)">' + esc(k) + '</span>=<span style="color:var(--amber-light)">' + esc(v) + '</span>')
        .join(' ');
      return '<tr>'
        + '<td style="color:var(--green-light)">' + esc(c.name) + '</td>'
        + '<td style="color:var(--text-muted);font-size:10px">' + esc(c.help) + '</td>'
        + '<td style="color:var(--amber);font-weight:bold">' + fmt(c.value) + '</td>'
        + '<td>' + (labelStr || '—') + '</td>'
        + '</tr>';
    }).join('');
    container.innerHTML = '<table>'
      + '<thead><tr><th>Metric</th><th>Help</th><th>Value</th><th>Labels</th></tr></thead>'
      + '<tbody>' + rows + '</tbody>'
      + '</table>';
  } catch (e) {
    container.innerHTML = '<p style="color:var(--red-bf)">' + esc(e.message) + '</p>';
    toast('Stats fetch failed: ' + e.message, 'err');
  }
}

document.getElementById('btn-reset-counters').addEventListener('click', async () => {
  try {
    await apiFetch('/reset_counters', { method: 'POST' });
    toast('Counters reset', 'ok');
    refreshStats();
  } catch (e) {
    toast('Reset failed: ' + e.message, 'err');
  }
});

// -- config dump page --

async function refreshConfigDump() {
  const pre = document.getElementById('config-dump-pre');
  try {
    const data = await apiFetch('/config_dump');
    // Pretty-print as JSON (YAML not available client-side without a lib)
    pre.textContent = JSON.stringify(data.config, null, 2);
  } catch (e) {
    pre.textContent = 'Error: ' + e.message;
    toast('Config dump failed: ' + e.message, 'err');
  }
}

document.getElementById('btn-config-reload').addEventListener('click', async () => {
  const btn = document.getElementById('btn-config-reload');
  btn.disabled = true;
  btn.textContent = 'Reloading...';
  try {
    const data = await apiFetch('/config/reload', { method: 'POST' });
    toast('Config reloaded — diff: ' + JSON.stringify(data.diff), 'ok');
    refreshConfigDump();
  } catch (e) {
    toast('Reload failed: ' + e.message, 'err');
  } finally {
    btn.disabled = false;
    btn.textContent = 'Reload Config';
  }
});

// -- polling loop --

function refreshPage() {
  if      (activeTab === 'overview') refreshOverview();
  else if (activeTab === 'clusters') refreshClusters();
  else if (activeTab === 'listeners') refreshListeners();
  else if (activeTab === 'stats')    refreshStats();
  else if (activeTab === 'config')   refreshConfigDump();
}

function startPolling() {
  if (pollHandle) clearInterval(pollHandle);
  pollHandle = setInterval(() => {
    if (activeTab === 'overview' || activeTab === 'stats') refreshPage();
    // Update uptime on overview without fetch
    const el = document.getElementById('stat-uptime');
    if (el) el.textContent = fmtUptime(startTime);
    // Blink poll indicator
    const ind = document.getElementById('poll-dot');
    if (ind) { ind.style.color = 'var(--amber)'; setTimeout(() => { if(ind) ind.style.color='var(--text-muted)'; }, 300); }
  }, POLL_MS);
}

// -- init --

document.addEventListener('DOMContentLoaded', () => {
  showTab('overview');
  startPolling();
});
