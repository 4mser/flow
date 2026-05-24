const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let status = null;

async function init() {
  try {
    status = await invoke("get_status");
    renderStatus();
    renderMonitors();
    setupListeners();
    updateBadge("online", "online");
  } catch (e) {
    console.error("Init error:", e);
    updateBadge("error", e.toString());
  }
}

function renderStatus() {
  document.getElementById("my-name").textContent = status.name;
  document.getElementById("my-os").textContent = status.os;
  document.getElementById("my-monitors").textContent = status.monitors.length;
  document.getElementById("my-peer-id").textContent = status.peer_id;
  renderPeers(status.peers);
}

function renderMonitors() {
  const canvas = document.getElementById("monitor-canvas");
  canvas.innerHTML = "";

  const allMonitors = [];

  status.monitors.forEach((m, i) => {
    allMonitors.push({ ...m, owner: status.name, mine: true, color: status.color });
  });

  status.peers.forEach((peer) => {
    if (peer.monitors) {
      peer.monitors.forEach((m) => {
        allMonitors.push({ ...m, owner: peer.name, mine: false, color: peer.color });
      });
    }
  });

  if (allMonitors.length === 0) return;

  const minX = Math.min(...allMonitors.map((m) => m.x));
  const minY = Math.min(...allMonitors.map((m) => m.y));
  const maxX = Math.max(...allMonitors.map((m) => m.x + m.width));
  const maxY = Math.max(...allMonitors.map((m) => m.y + m.height));

  const totalW = maxX - minX;
  const totalH = maxY - minY;

  const canvasRect = canvas.getBoundingClientRect();
  const padding = 40;
  const availW = canvasRect.width - padding * 2;
  const availH = canvasRect.height - padding * 2;
  const scale = Math.min(availW / totalW, availH / totalH, 0.15);

  allMonitors.forEach((m) => {
    const el = document.createElement("div");
    el.className = `monitor-block ${m.mine ? "mine" : "peer"}`;
    el.style.width = m.width * scale + "px";
    el.style.height = m.height * scale + "px";
    el.style.position = "absolute";
    el.style.left = padding + (m.x - minX) * scale + "px";
    el.style.top = padding + (m.y - minY) * scale + "px";

    if (!m.mine) {
      el.style.borderColor = m.color;
      el.style.boxShadow = `0 0 20px ${m.color}22`;
    }

    el.innerHTML = `
      <span class="monitor-label">${m.name}</span>
      <span class="monitor-res">${m.width}x${m.height}</span>
      <span class="monitor-owner">${m.owner}</span>
    `;
    canvas.appendChild(el);
  });
}

function renderPeers(peers) {
  const list = document.getElementById("peers-list");
  const count = document.getElementById("peer-count");
  count.textContent = peers.length;

  if (peers.length === 0) {
    list.innerHTML = '<div class="empty-state">No peers connected</div>';
    return;
  }

  list.innerHTML = peers
    .map(
      (p) => `
    <div class="peer-item">
      <div class="peer-dot" style="background: ${p.color}"></div>
      <div class="peer-info">
        <div class="peer-name">${p.name}</div>
        <div class="peer-os">${p.os} · ${p.peer_id}</div>
      </div>
    </div>
  `
    )
    .join("");
}

function updateBadge(cls, text) {
  const badge = document.getElementById("status-badge");
  badge.className = `badge ${cls}`;
  badge.textContent = text;
}

async function connectPeer() {
  const input = document.getElementById("peer-ip");
  const btn = document.getElementById("connect-btn");
  const ip = input.value.trim();

  if (!ip) return;

  btn.disabled = true;
  btn.textContent = "Connecting...";

  try {
    const result = await invoke("connect_peer", { ip });
    input.value = "";
    addClipboardEntry("connected", `Connected to ${ip}`, "sent");

    const peers = await invoke("get_peers");
    renderPeers(peers);
  } catch (e) {
    addClipboardEntry("error", `Failed: ${e}`, "received");
  } finally {
    btn.disabled = false;
    btn.textContent = "Connect";
  }
}

function addClipboardEntry(direction, text, cls) {
  const log = document.getElementById("clipboard-log");

  const empty = log.querySelector(".empty-state");
  if (empty) empty.remove();

  const now = new Date();
  const time = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });

  const entry = document.createElement("div");
  entry.className = "clip-entry";
  entry.innerHTML = `
    <span class="clip-dir ${cls}">${direction === "sent" ? "SENT" : "RECV"}</span>
    <span class="clip-text">${escapeHtml(text.substring(0, 100))}</span>
    <span class="clip-time">${time}</span>
  `;

  log.insertBefore(entry, log.firstChild);

  while (log.children.length > 50) {
    log.removeChild(log.lastChild);
  }
}

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

async function setupListeners() {
  await listen("peer-joined", (event) => {
    const peer = event.payload;
    addClipboardEntry("connected", `${peer.name} joined (${peer.os})`, "received");
    invoke("get_peers").then(renderPeers);
    invoke("get_status").then((s) => {
      status = s;
      renderMonitors();
    });
    updateBadge("online", `${peer.name} connected`);
    setTimeout(() => updateBadge("online", "online"), 3000);
  });

  await listen("peer-left", (event) => {
    addClipboardEntry("disconnected", `Peer left: ${event.payload}`, "sent");
    invoke("get_peers").then(renderPeers);
  });

  await listen("clipboard-sent", (event) => {
    addClipboardEntry("sent", event.payload, "sent");
  });

  await listen("clipboard-received", (event) => {
    addClipboardEntry("received", event.payload, "received");
  });
}

document.getElementById("peer-ip").addEventListener("keydown", (e) => {
  if (e.key === "Enter") connectPeer();
});

document.addEventListener("DOMContentLoaded", init);
