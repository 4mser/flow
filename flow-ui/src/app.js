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
  document.getElementById("my-ip").textContent = status.local_ip;
  document.getElementById("local-ip").textContent = status.peer_id;
  document.getElementById("my-monitors").textContent =
    status.monitors.length + " display" + (status.monitors.length !== 1 ? "s" : "");
  renderPeers(status.peers);
}

function renderMonitors() {
  var canvas = document.getElementById("monitor-canvas");
  canvas.innerHTML = "";

  var all = [];
  status.monitors.forEach(function (m) {
    all.push({ name: m.name, width: m.width, height: m.height, x: m.x, y: m.y, owner: status.name, mine: true, color: status.color });
  });
  status.peers.forEach(function (p) {
    if (p.monitors) {
      p.monitors.forEach(function (m) {
        all.push({ name: m.name, width: m.width, height: m.height, x: m.x, y: m.y, owner: p.name, mine: false, color: p.color });
      });
    }
  });

  if (all.length === 0) {
    canvas.innerHTML = '<div class="empty-state">No monitors</div>';
    return;
  }

  var minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  all.forEach(function (m) {
    if (m.x < minX) minX = m.x;
    if (m.y < minY) minY = m.y;
    if (m.x + m.width > maxX) maxX = m.x + m.width;
    if (m.y + m.height > maxY) maxY = m.y + m.height;
  });

  var totalW = maxX - minX || 1;
  var totalH = maxY - minY || 1;
  var rect = canvas.getBoundingClientRect();
  var availW = rect.width - 40;
  var availH = rect.height - 40;
  if (availW <= 0 || availH <= 0) return;

  var scale = Math.min(availW / totalW, availH / totalH);
  var renderedW = totalW * scale;
  var renderedH = totalH * scale;
  var offsetX = (rect.width - renderedW) / 2;
  var offsetY = (rect.height - renderedH) / 2;

  all.forEach(function (m) {
    var el = document.createElement("div");
    el.className = "monitor-block " + (m.mine ? "mine" : "peer");
    var w = m.width * scale;
    var h = m.height * scale;
    el.style.width = w + "px";
    el.style.height = h + "px";
    el.style.left = (offsetX + (m.x - minX) * scale) + "px";
    el.style.top = (offsetY + (m.y - minY) * scale) + "px";
    if (!m.mine) el.style.borderColor = m.color;

    var showRes = w > 80 && h > 50;
    el.innerHTML =
      '<span class="monitor-label">' + esc(m.name) + '</span>' +
      (showRes ? '<span class="monitor-res">' + m.width + 'x' + m.height + '</span>' : '') +
      '<span class="monitor-owner">' + esc(m.owner) + '</span>';
    canvas.appendChild(el);
  });
}

function renderPeers(peers) {
  var list = document.getElementById("peers-list");
  var count = document.getElementById("peer-count");
  count.textContent = peers.length;

  if (peers.length === 0) {
    list.innerHTML = '<div class="empty-state">Waiting for peers...</div>';
    document.getElementById("send-file-btn").disabled = true;
    return;
  }

  document.getElementById("send-file-btn").disabled = false;
  list.innerHTML = peers.map(function (p) {
    return '<div class="peer-item">' +
      '<div class="peer-dot" style="background:' + esc(p.color) + '"></div>' +
      '<div class="peer-info">' +
        '<div class="peer-name">' + esc(p.name) + '</div>' +
        '<div class="peer-os">' + esc(p.os) + ' · ' + esc(p.peer_id) + '</div>' +
      '</div>' +
    '</div>';
  }).join("");
}

function updateBadge(cls, text) {
  var badge = document.getElementById("status-badge");
  badge.className = "badge " + cls;
  badge.textContent = text;
}

async function connectPeer() {
  var input = document.getElementById("peer-ip");
  var btn = document.getElementById("connect-btn");
  var ip = input.value.trim();
  if (!ip) return;

  btn.disabled = true;
  btn.textContent = "...";

  try {
    await invoke("connect_peer", { ip: ip });
    input.value = "";
    addClip("sent", "Connected to " + ip);
    var peers = await invoke("get_peers");
    renderPeers(peers);
  } catch (e) {
    addClip("received", "Failed: " + e);
  } finally {
    btn.disabled = false;
    btn.textContent = "Connect";
  }
}

async function pickAndSend() {
  var btn = document.getElementById("send-file-btn");
  var statusEl = document.getElementById("file-status");
  btn.disabled = true;
  statusEl.textContent = "Selecting file...";

  try {
    var name = await invoke("pick_and_send");
    statusEl.textContent = "Sent: " + name;
    addClip("sent", "File sent: " + name);
    setTimeout(function () { statusEl.textContent = ""; }, 5000);
  } catch (e) {
    statusEl.textContent = e === "No file selected" ? "" : "Error: " + e;
  } finally {
    btn.disabled = false;
  }
}

function addClip(dir, text) {
  var log = document.getElementById("clipboard-log");
  var empty = log.querySelector(".empty-state");
  if (empty) empty.remove();

  var now = new Date();
  var t = pad2(now.getHours()) + ":" + pad2(now.getMinutes()) + ":" + pad2(now.getSeconds());

  var entry = document.createElement("div");
  entry.className = "clip-entry";
  entry.innerHTML =
    '<span class="clip-dir ' + dir + '">' + (dir === "sent" ? "SENT" : "RECV") + '</span>' +
    '<span class="clip-text">' + esc(text.substring(0, 120)) + '</span>' +
    '<span class="clip-time">' + t + '</span>';

  log.insertBefore(entry, log.firstChild);
  while (log.children.length > 30) log.removeChild(log.lastChild);
}

function pad2(n) { return n < 10 ? "0" + n : "" + n; }
function esc(s) { var d = document.createElement("div"); d.textContent = s; return d.innerHTML; }

async function setupListeners() {
  await listen("peer-joined", function (e) {
    var peer = e.payload;
    addClip("received", peer.name + " joined (" + peer.os + ")");
    invoke("get_peers").then(renderPeers);
    invoke("get_status").then(function (s) { status = s; renderMonitors(); });
    updateBadge("online", peer.name + " connected");
    setTimeout(function () { updateBadge("online", "online"); }, 3000);
  });

  await listen("peer-left", function (e) {
    addClip("sent", "Peer left: " + e.payload);
    invoke("get_peers").then(renderPeers);
  });

  await listen("clipboard-sent", function (e) { addClip("sent", e.payload); });
  await listen("clipboard-received", function (e) { addClip("received", e.payload); });

  await listen("file-incoming", function (e) { addClip("received", "Incoming: " + e.payload); });
  await listen("file-received", function (e) {
    addClip("received", "File saved: " + e.payload);
    document.getElementById("file-status").textContent = "Received: " + e.payload;
  });
  await listen("file-sent", function (e) { addClip("sent", "File sent: " + e.payload); });
  await listen("file-complete", function (e) { addClip("received", "Transfer done: " + e.payload); });
}

document.getElementById("peer-ip").addEventListener("keydown", function (e) {
  if (e.key === "Enter") connectPeer();
});

window.addEventListener("resize", function () {
  if (status) renderMonitors();
});

document.addEventListener("DOMContentLoaded", init);
