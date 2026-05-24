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
    updateBadge("", "error");
  }
}

function renderStatus() {
  document.getElementById("my-name").textContent = status.name;
  document.getElementById("my-os").textContent = status.os;
  document.getElementById("my-monitors").textContent = status.monitors.length + " display" + (status.monitors.length !== 1 ? "s" : "");
  document.getElementById("my-peer-id").textContent = status.peer_id;
  renderPeers(status.peers);
}

function renderMonitors() {
  const canvas = document.getElementById("monitor-canvas");
  canvas.innerHTML = "";

  const allMonitors = [];

  status.monitors.forEach((m) => {
    allMonitors.push({ ...m, owner: status.name, mine: true, color: status.color });
  });

  status.peers.forEach((peer) => {
    if (peer.monitors) {
      peer.monitors.forEach((m) => {
        allMonitors.push({ ...m, owner: peer.name, mine: false, color: peer.color });
      });
    }
  });

  if (allMonitors.length === 0) {
    canvas.innerHTML = '<div class="empty-state">No monitors detected</div>';
    return;
  }

  const minX = Math.min(...allMonitors.map((m) => m.x));
  const minY = Math.min(...allMonitors.map((m) => m.y));
  const maxX = Math.max(...allMonitors.map((m) => m.x + m.width));
  const maxY = Math.max(...allMonitors.map((m) => m.y + m.height));

  const totalW = maxX - minX || 1;
  const totalH = maxY - minY || 1;

  const rect = canvas.getBoundingClientRect();
  const pad = 20;
  const availW = rect.width - pad * 2;
  const availH = rect.height - pad * 2;

  if (availW <= 0 || availH <= 0) return;

  const scale = Math.min(availW / totalW, availH / totalH);

  const renderedW = totalW * scale;
  const renderedH = totalH * scale;
  const offsetX = (rect.width - renderedW) / 2;
  const offsetY = (rect.height - renderedH) / 2;

  allMonitors.forEach((m) => {
    const el = document.createElement("div");
    el.className = "monitor-block " + (m.mine ? "mine" : "peer");

    const w = m.width * scale;
    const h = m.height * scale;
    const x = offsetX + (m.x - minX) * scale;
    const y = offsetY + (m.y - minY) * scale;

    el.style.width = w + "px";
    el.style.height = h + "px";
    el.style.left = x + "px";
    el.style.top = y + "px";

    if (!m.mine) {
      el.style.borderColor = m.color;
    }

    const showRes = w > 80 && h > 50;

    el.innerHTML =
      '<span class="monitor-label">' + esc(m.name) + '</span>' +
      (showRes ? '<span class="monitor-res">' + m.width + 'x' + m.height + '</span>' : '') +
      '<span class="monitor-owner">' + esc(m.owner) + '</span>';

    canvas.appendChild(el);
  });
}

function renderPeers(peers) {
  const list = document.getElementById("peers-list");
  const count = document.getElementById("peer-count");
  count.textContent = peers.length;

  if (peers.length === 0) {
    list.innerHTML = '<div class="empty-state">Waiting for peers...</div>';
    return;
  }

  list.innerHTML = peers
    .map(function (p) {
      return '<div class="peer-item">' +
        '<div class="peer-dot" style="background:' + esc(p.color) + '"></div>' +
        '<div class="peer-info">' +
          '<div class="peer-name">' + esc(p.name) + '</div>' +
          '<div class="peer-os">' + esc(p.os) + ' · ' + esc(p.peer_id) + '</div>' +
        '</div>' +
      '</div>';
    })
    .join("");
}

function updateBadge(cls, text) {
  const badge = document.getElementById("status-badge");
  badge.className = "badge " + cls;
  badge.textContent = text;
}

async function connectPeer() {
  const input = document.getElementById("peer-ip");
  const btn = document.getElementById("connect-btn");
  const ip = input.value.trim();
  if (!ip) return;

  btn.disabled = true;
  btn.textContent = "...";

  try {
    await invoke("connect_peer", { ip: ip });
    input.value = "";
    addClip("sent", "Connected to " + ip);
    const peers = await invoke("get_peers");
    renderPeers(peers);
  } catch (e) {
    addClip("received", "Failed: " + e);
  } finally {
    btn.disabled = false;
    btn.textContent = "Connect";
  }
}

function addClip(dir, text) {
  const log = document.getElementById("clipboard-log");
  const empty = log.querySelector(".empty-state");
  if (empty) empty.remove();

  const now = new Date();
  const t = pad2(now.getHours()) + ":" + pad2(now.getMinutes()) + ":" + pad2(now.getSeconds());

  const entry = document.createElement("div");
  entry.className = "clip-entry";
  entry.innerHTML =
    '<span class="clip-dir ' + dir + '">' + (dir === "sent" ? "SENT" : "RECV") + '</span>' +
    '<span class="clip-text">' + esc(text.substring(0, 120)) + '</span>' +
    '<span class="clip-time">' + t + '</span>';

  log.insertBefore(entry, log.firstChild);

  while (log.children.length > 30) {
    log.removeChild(log.lastChild);
  }
}

function pad2(n) {
  return n < 10 ? "0" + n : "" + n;
}

function esc(s) {
  var d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}

async function setupListeners() {
  await listen("peer-joined", function (event) {
    var peer = event.payload;
    addClip("received", peer.name + " joined (" + peer.os + ")");
    invoke("get_peers").then(renderPeers);
    invoke("get_status").then(function (s) {
      status = s;
      renderMonitors();
    });
    updateBadge("online", peer.name + " connected");
    setTimeout(function () { updateBadge("online", "online"); }, 3000);
  });

  await listen("peer-left", function (event) {
    addClip("sent", "Peer left: " + event.payload);
    invoke("get_peers").then(renderPeers);
  });

  await listen("clipboard-sent", function (event) {
    addClip("sent", event.payload);
  });

  await listen("clipboard-received", function (event) {
    addClip("received", event.payload);
  });
}

document.getElementById("peer-ip").addEventListener("keydown", function (e) {
  if (e.key === "Enter") connectPeer();
});

window.addEventListener("resize", function () {
  if (status) renderMonitors();
});

document.addEventListener("DOMContentLoaded", init);
