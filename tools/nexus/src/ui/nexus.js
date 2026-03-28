/* Nyx Nexus — Production Frontend v1.0 */
'use strict';

// ── State ────────────────────────────────────────────────────────────────────
const State = {
  vitals:      null,
  status:      null,
  health:      null,
  files:       [],
  diagnostics: [],
  modules:     null,
  cpuHistory:  new Array(60).fill(0),
  diagFilter:  'all',
  fileSearch:  '',
  ws:          null,
};

// ── Navigation ───────────────────────────────────────────────────────────────
document.querySelectorAll('.nav-item').forEach(link => {
  link.addEventListener('click', e => {
    e.preventDefault();
    const section = link.dataset.section;
    activateSection(section);
    link.closest('.sidebar-nav').querySelectorAll('.nav-item').forEach(l => l.classList.remove('active'));
    link.classList.add('active');
  });
});

function activateSection(id) {
  document.querySelectorAll('.panel').forEach(p => p.classList.remove('active'));
  const panel = document.getElementById(`panel-${id}`);
  if (panel) panel.classList.add('active');
  document.getElementById('section-title').textContent =
    ({overview:'Overview', health:'Health Audit', diagnostics:'Diagnostics',
      modules:'Architecture', files:'File Explorer', vitals:'System Vitals'})[id] || id;
}

// ── WebSocket ─────────────────────────────────────────────────────────────────
function connectWs() {
  const wsUrl = `ws://${location.host}/ws`;
  const dot = document.querySelector('.ws-dot');
  const lbl = document.getElementById('ws-label');

  try {
    State.ws = new WebSocket(wsUrl);

    State.ws.onopen = () => {
      dot.className = 'ws-dot connected';
      lbl.textContent = 'Live';
    };

    State.ws.onmessage = ({ data }) => {
      try {
        const ev = JSON.parse(data);
        if (ev.kind === 'vitals') {
          patchVitals(ev.payload);
        }
      } catch (err) { /* ignore parse errors */ }
    };

    State.ws.onclose = () => {
      dot.className = 'ws-dot error';
      lbl.textContent = 'Disconnected';
      setTimeout(connectWs, 4000);
    };

    State.ws.onerror = () => {
      dot.className = 'ws-dot error';
      lbl.textContent = 'Error';
    };
  } catch (e) {
    lbl.textContent = 'N/A';
  }
}

function patchVitals(partial) {
  // Update topbar CPU/RAM live from WS
  set('tb-cpu', Math.round(partial.cpu_usage ?? 0));
  set('tb-ram', partial.memory_used_mb ?? '--');

  // Push to CPU history
  State.cpuHistory.push(partial.cpu_usage ?? 0);
  if (State.cpuHistory.length > 60) State.cpuHistory.shift();
  drawCpuChart();

  // Update heartbeat bars in overview
  updateHbBar('hb-cpu', 'hb-cpu-val', partial.cpu_usage, 100, '%');
  updateHbBar('hb-ram', 'hb-ram-val', partial.memory_used_mb, State.vitals?.memory_total_mb || 8192, 'MB');
}

// ── Fetch Helpers ─────────────────────────────────────────────────────────────
async function api(path) {
  const res = await fetch(path);
  if (!res.ok) throw new Error(`${path} → ${res.status}`);
  return res.json();
}

// ── Main Refresh ──────────────────────────────────────────────────────────────
async function refreshAll() {
  try {
    const [status, health, files, vitals, diags, mods] = await Promise.all([
      api('/api/status'),
      api('/api/health'),
      api('/api/files'),
      api('/api/vitals'),
      api('/api/diagnostics'),
      api('/api/modules'),
    ]);

    State.status      = status;
    State.health      = health;
    State.files       = files;
    State.vitals      = vitals;
    State.diagnostics = diags;
    State.modules     = mods;

    renderAll();
  } catch (err) {
    console.error('[Nexus] Sync error:', err);
  }
}

function renderAll() {
  renderOverview();
  renderHealth();
  renderDiagnostics();
  renderFiles();
  renderVitals();
  renderModules();
}

// ── Overview ──────────────────────────────────────────────────────────────────
function renderOverview() {
  const s = State.status;
  const v = State.vitals;
  if (!s || !v) return;

  set('ov-health', s.health_score + '%');
  set('ov-files',  s.file_count);
  set('ov-nyx',    s.nyx_file_count);
  set('ov-rs',     s.rs_file_count);
  set('ov-lines',  s.total_lines.toLocaleString());
  set('ov-procs',  v.process_count);

  const bar = document.getElementById('ov-health-bar');
  if (bar) setTimeout(() => { bar.style.width = s.health_score + '%'; }, 100);

  // Uptime
  const up = s.uptime_secs;
  const h = Math.floor(up/3600), m = Math.floor((up%3600)/60), sec = up%60;
  set('tb-uptime', `${h}h ${m}m ${sec}s`);

  // Topbar
  set('tb-cpu', Math.round(v.cpu_usage));
  set('tb-ram', v.memory_used_mb);

  // Heartbeat bars
  updateHbBar('hb-cpu',  'hb-cpu-val',  v.cpu_usage,       100,                    '%');
  updateHbBar('hb-ram',  'hb-ram-val',  v.memory_used_mb,  v.memory_total_mb,      'MB');
  updateHbBar('hb-disk', 'hb-disk-val', v.disk_used_gb,    v.disk_total_gb,        'GB');

  // Quick health mini list
  if (State.health) {
    const top5 = State.health.checks.slice(0, 5);
    document.getElementById('ov-health-list').innerHTML = top5.map(c => `
      <li class="mini-check">
        <span class="mini-check-name">${c.name}</span>
        <span class="badge ${c.status ? 'badge-ok' : (c.critical ? 'badge-fail' : 'badge-warn')}">
          ${c.status ? 'OK' : 'FAIL'}
        </span>
      </li>
    `).join('');
  }
}

function updateHbBar(barId, valId, value, max, unit) {
  const bar = document.getElementById(barId);
  const val = document.getElementById(valId);
  if (!bar || !val || max === 0) return;
  const pct = Math.min(100, (value / max) * 100);
  bar.style.width = pct + '%';
  val.textContent = unit === '%' ? Math.round(value) + '%' : Math.round(value) + ' ' + unit;
}

// ── Health Audit ──────────────────────────────────────────────────────────────
function renderHealth() {
  const h = State.health;
  if (!h) return;

  set('health-score-badge', `${h.overall_score}/100`);

  document.getElementById('health-table').innerHTML = h.checks.map(c => `
    <div class="health-row">
      <span class="name">${c.name}${c.critical ? ' <span style="color:var(--red);font-size:0.65rem">CRITICAL</span>' : ''}</span>
      <span class="msg">${c.message}</span>
      <span class="cat">${c.category}</span>
      <span class="badge ${c.status ? 'badge-ok' : (c.critical ? 'badge-fail' : 'badge-warn')}">${c.status ? 'PASS' : 'FAIL'}</span>
    </div>
  `).join('');
}

// ── Diagnostics ───────────────────────────────────────────────────────────────
function renderDiagnostics() {
  const list = document.getElementById('diag-list');
  let items = State.diagnostics;
  if (State.diagFilter !== 'all') {
    items = items.filter(d => d.kind === State.diagFilter);
  }
  if (items.length === 0) {
    list.innerHTML = '<div class="loading-msg">No diagnostics found ✓</div>';
    return;
  }
  list.innerHTML = items.map(d => {
    const sev = d.severity === 'warning' ? 'warning' : d.severity === 'error' ? 'error' : 'info';
    const loc = d.line ? `${shortPath(d.file)}:${d.line}` : shortPath(d.file);
    return `
      <div class="diag-item ${sev}">
        <span class="diag-id">${d.id}</span>
        <span class="diag-kind">${d.kind}</span>
        <span class="diag-msg">${d.message}</span>
        <span class="diag-loc">${loc}</span>
      </div>
    `;
  }).join('');
}

document.getElementById('diag-filters').addEventListener('click', e => {
  const btn = e.target.closest('.filter-btn');
  if (!btn) return;
  State.diagFilter = btn.dataset.filter;
  document.querySelectorAll('.filter-btn').forEach(b => b.classList.remove('active'));
  btn.classList.add('active');
  renderDiagnostics();
});

// ── Files ─────────────────────────────────────────────────────────────────────
function renderFiles() {
  const search = State.fileSearch.toLowerCase();
  let files = State.files.filter(f => !search || f.name.toLowerCase().includes(search));
  if (files.length === 0) {
    document.getElementById('file-table').innerHTML = '<div class="loading-msg">No files found</div>';
    return;
  }
  document.getElementById('file-table').innerHTML = files.map(f => {
    const kb = (f.size / 1024).toFixed(1);
    const extClass = `ext-${f.extension}`;
    const t = new Date(f.modified_secs * 1000).toLocaleDateString();
    return `
      <div class="file-row">
        <span class="file-name">
          <span class="ext-badge ${extClass}">.${f.extension}</span>
          ${f.name}
        </span>
        <span class="file-ext">${f.extension.toUpperCase()}</span>
        <span class="file-size">${kb} KB</span>
        <span class="file-time">${t}</span>
      </div>
    `;
  }).join('');
}

document.getElementById('file-search').addEventListener('input', e => {
  State.fileSearch = e.target.value;
  renderFiles();
});

// ── Vitals ────────────────────────────────────────────────────────────────────
function renderVitals() {
  const v = State.vitals;
  if (!v) return;
  set('v-cpu',   Math.round(v.cpu_usage));
  set('v-ram',   v.memory_used_mb + ' MB');
  set('v-ram-pct', `${Math.round(v.memory_percent)}% of ${v.memory_total_mb} MB`);
  set('v-disk',  v.disk_used_gb.toFixed(1) + ' GB');
  set('v-disk-total', `of ${v.disk_total_gb.toFixed(1)} GB`);
  set('v-load',  v.load_avg.toFixed(2));

  State.cpuHistory.push(v.cpu_usage);
  if (State.cpuHistory.length > 60) State.cpuHistory.shift();
  drawCpuChart();
}

// ── Modules ───────────────────────────────────────────────────────────────────
function renderModules() {
  const g = State.modules;
  if (!g || !g.nodes?.length) {
    document.getElementById('module-table').innerHTML = '<div class="loading-msg">No modules found</div>';
    return;
  }

  // Canvas-based graph
  drawModuleGraph(g);

  // Module index table
  document.getElementById('module-table').innerHTML = g.nodes.map(n => `
    <div class="module-pill">
      <span class="mod-name">${n.label || n.id}</span>
      <span class="mod-files">${n.file_count} files</span>
    </div>
  `).join('');
}

const GROUP_COLORS = ['#8b5cf6','#10b981','#3b82f6','#f59e0b','#ef4444','#06b6d4','#d946ef'];

function drawModuleGraph(g) {
  const canvas = document.getElementById('module-canvas');
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const W = canvas.offsetWidth;
  const H = 400;
  canvas.width = W;
  canvas.height = H;

  const n = g.nodes.length;
  if (n === 0) return;

  // Position nodes in a circle
  const cx = W / 2, cy = H / 2;
  const r = Math.min(cx, cy) - 70;
  const positions = g.nodes.map((node, i) => ({
    x: cx + r * Math.cos((2 * Math.PI * i) / n - Math.PI / 2),
    y: cy + r * Math.sin((2 * Math.PI * i) / n - Math.PI / 2),
    node,
  }));

  const nodeById = Object.fromEntries(positions.map(p => [p.node.id, p]));

  // Draw edges
  ctx.strokeStyle = 'rgba(139,92,246,0.2)';
  ctx.lineWidth = 1.5;
  for (const link of g.links) {
    const src = nodeById[link.source];
    const tgt = nodeById[link.target];
    if (!src || !tgt) continue;
    ctx.beginPath();
    ctx.moveTo(src.x, src.y);
    ctx.lineTo(tgt.x, tgt.y);
    ctx.stroke();
  }

  // Draw nodes
  for (const { x, y, node } of positions) {
    const color = GROUP_COLORS[(node.group - 1) % GROUP_COLORS.length];
    // Glow
    const grd = ctx.createRadialGradient(x, y, 0, x, y, 32);
    grd.addColorStop(0, color + '44');
    grd.addColorStop(1, 'transparent');
    ctx.fillStyle = grd;
    ctx.beginPath(); ctx.arc(x, y, 32, 0, Math.PI * 2); ctx.fill();
    // Circle
    ctx.beginPath(); ctx.arc(x, y, 16, 0, Math.PI * 2);
    ctx.fillStyle = color + 'cc';
    ctx.fill();
    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    ctx.stroke();
    // Label
    ctx.fillStyle = '#f1f5f9';
    ctx.font = '600 11px Inter, sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(node.label || node.id, x, y + 30);
  }

  // Legend
  const legend = document.getElementById('module-legend');
  const groups = [...new Set(g.nodes.map(n => n.group))];
  legend.innerHTML = groups.map(gr => `
    <div class="legend-item">
      <div class="legend-dot" style="background:${GROUP_COLORS[(gr-1)%GROUP_COLORS.length]}"></div>
      Group ${gr}
    </div>
  `).join('');
}

// ── CPU History Chart ─────────────────────────────────────────────────────────
function drawCpuChart() {
  const canvas = document.getElementById('cpu-chart');
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const W = canvas.offsetWidth;
  const H = 120;
  canvas.width = W;
  canvas.height = H;
  ctx.clearRect(0, 0, W, H);

  const data = State.cpuHistory;
  const max = 100;
  const step = W / (data.length - 1);

  // Gradient fill
  const grad = ctx.createLinearGradient(0, 0, 0, H);
  grad.addColorStop(0, 'rgba(139,92,246,0.5)');
  grad.addColorStop(1, 'rgba(139,92,246,0)');

  ctx.beginPath();
  ctx.moveTo(0, H - (data[0] / max) * H);
  data.forEach((v, i) => ctx.lineTo(i * step, H - (v / max) * H));
  ctx.lineTo(W, H); ctx.lineTo(0, H); ctx.closePath();
  ctx.fillStyle = grad;
  ctx.fill();

  // Line
  ctx.beginPath();
  ctx.moveTo(0, H - (data[0] / max) * H);
  data.forEach((v, i) => ctx.lineTo(i * step, H - (v / max) * H));
  ctx.strokeStyle = '#8b5cf6';
  ctx.lineWidth = 2;
  ctx.stroke();
}

// ── Utilities ─────────────────────────────────────────────────────────────────
function set(id, val) {
  const el = document.getElementById(id);
  if (el) el.textContent = val;
}

function shortPath(p) {
  const parts = p.replace(/\\/g, '/').split('/');
  return parts.slice(-2).join('/');
}

// ── Refresh button ────────────────────────────────────────────────────────────
document.getElementById('refresh-btn').addEventListener('click', () => {
  document.getElementById('refresh-btn').style.transform = 'rotate(360deg)';
  refreshAll().then(() => {
    setTimeout(() => { document.getElementById('refresh-btn').style.transform = ''; }, 400);
  });
});

// ── Bootstrap ────────────────────────────────────────────────────────────────
document.addEventListener('DOMContentLoaded', () => {
  connectWs();
  refreshAll();
  // Refresh data every 5s
  setInterval(refreshAll, 5000);
});
