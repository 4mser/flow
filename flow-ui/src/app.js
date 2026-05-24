const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let status = null;
let layoutMonitors = [];
let dragState = null;
let canvasScale = 1;
let canvasOffsetX = 0;
let canvasOffsetY = 0;
let canvasMinX = 0;
let canvasMinY = 0;

async function init() {
  try {
    status = await invoke("get_status");
    renderStatus();
    await loadLayout();
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

async function loadLayout() {
  layoutMonitors = await invoke("get_layout");
  renderMonitors();
}

function renderMonitors() {
  var canvas = document.getElementById("monitor-canvas");
  canvas.innerHTML = "";

  if (layoutMonitors.length === 0) {
    canvas.innerHTML = '<div class="empty-state">No monitors</div>';
    return;
  }

  var minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  layoutMonitors.forEach(function (m) {
    if (m.x < minX) minX = m.x;
    if (m.y < minY) minY = m.y;
    if (m.x + m.width > maxX) maxX = m.x + m.width;
    if (m.y + m.height > maxY) maxY = m.y + m.height;
  });

  canvasMinX = minX;
  canvasMinY = minY;
  var totalW = maxX - minX || 1;
  var totalH = maxY - minY || 1;
  var rect = canvas.getBoundingClientRect();
  var availW = rect.width - 60;
  var availH = rect.height - 60;
  if (availW <= 0 || availH <= 0) return;

  canvasScale = Math.min(availW / totalW, availH / totalH);
  var renderedW = totalW * canvasScale;
  var renderedH = totalH * canvasScale;
  canvasOffsetX = (rect.width - renderedW) / 2;
  canvasOffsetY = (rect.height - renderedH) / 2;

  layoutMonitors.forEach(function (m, idx) {
    var el = document.createElement("div");
    el.className = "monitor-block " + (m.mine ? "mine" : "peer");
    el.dataset.idx = idx;

    var w = m.width * canvasScale;
    var h = m.height * canvasScale;
    el.style.width = w + "px";
    el.style.height = h + "px";
    el.style.left = (canvasOffsetX + (m.x - canvasMinX) * canvasScale) + "px";
    el.style.top = (canvasOffsetY + (m.y - canvasMinY) * canvasScale) + "px";
    el.style.cursor = "grab";

    if (!m.mine) el.style.borderColor = m.color;

    var showRes = w > 80 && h > 50;
    el.innerHTML =
      '<span class="monitor-label">' + esc(m.name) + '</span>' +
      (showRes ? '<span class="monitor-res">' + m.width + 'x' + m.height + '</span>' : '') +
      '<span class="monitor-owner">' + esc(m.peer_name) + '</span>';

    el.addEventListener("mousedown", startDrag);
    canvas.appendChild(el);
  });
}

function startDrag(e) {
  e.preventDefault();
  var idx = parseInt(e.currentTarget.dataset.idx);
  var el = e.currentTarget;
  el.style.cursor = "grabbing";
  el.style.zIndex = 10;

  dragState = {
    idx: idx,
    el: el,
    startMouseX: e.clientX,
    startMouseY: e.clientY,
    startLeft: parseFloat(el.style.left),
    startTop: parseFloat(el.style.top),
  };

  document.addEventListener("mousemove", onDrag);
  document.addEventListener("mouseup", endDrag);
}

function onDrag(e) {
  if (!dragState) return;
  var dx = e.clientX - dragState.startMouseX;
  var dy = e.clientY - dragState.startMouseY;
  dragState.el.style.left = (dragState.startLeft + dx) + "px";
  dragState.el.style.top = (dragState.startTop + dy) + "px";
}

function endDrag(e) {
  if (!dragState) return;
  document.removeEventListener("mousemove", onDrag);
  document.removeEventListener("mouseup", endDrag);

  var dx = e.clientX - dragState.startMouseX;
  var dy = e.clientY - dragState.startMouseY;

  var m = layoutMonitors[dragState.idx];
  m.x = m.x + Math.round(dx / canvasScale);
  m.y = m.y + Math.round(dy / canvasScale);

  dragState.el.style.cursor = "grab";
  dragState.el.style.zIndex = "";
  dragState = null;

  renderMonitors();
  showSaveBtn();
}

function showSaveBtn() {
  var btn = document.getElementById("save-layout-btn");
  if (btn) btn.style.display = "inline-block";
}

async function saveLayout() {
  var btn = document.getElementById("save-layout-btn");
  btn.textContent = "Saving...";
  btn.disabled = true;

  try {
    await invoke("save_layout", { monitors: layoutMonitors });
    btn.textContent = "Saved!";
    addClip("sent", "Monitor layout saved and synced");
    setTimeout(function () {
      btn.textContent = "Save Layout";
      btn.disabled = false;
      btn.style.display = "none";
    }, 2000);
  } catch (e) {
    btn.textContent = "Error";
    addClip("received", "Layout save failed: " + e);
    setTimeout(function () {
      btn.textContent = "Save Layout";
      btn.disabled = false;
    }, 2000);
  }
}

function renderPeers(peers) {
  var list = document.getElementById("peers-list");
  var count = document.getElementById("peer-count");
  count.textContent = peers.length;

  if (peers.length === 0) {
    list.innerHTML = '<div class="empty-state">Waiting for peers...</div>';
    var sfb = document.getElementById("send-file-btn");
    if (sfb) sfb.disabled = true;
    return;
  }

  var sfb = document.getElementById("send-file-btn");
  if (sfb) sfb.disabled = false;
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
    invoke("get_status").then(function (s) { status = s; });
    loadLayout();
    updateBadge("online", peer.name + " connected");
    setTimeout(function () { updateBadge("online", "online"); }, 3000);
  });

  await listen("peer-left", function (e) {
    addClip("sent", "Peer left: " + e.payload);
    invoke("get_peers").then(renderPeers);
  });

  await listen("layout-updated", function () { loadLayout(); });

  await listen("clipboard-sent", function (e) { addClip("sent", e.payload); });
  await listen("clipboard-received", function (e) { addClip("received", e.payload); });

  await listen("file-incoming", function (e) { addClip("received", "Incoming: " + e.payload); });
  await listen("file-received", function (e) {
    addClip("received", "File saved: " + e.payload);
    var fs = document.getElementById("file-status");
    if (fs) fs.textContent = "Received: " + e.payload;
  });
  await listen("file-sent", function (e) { addClip("sent", "File sent: " + e.payload); });
  await listen("file-complete", function (e) { addClip("received", "Transfer done: " + e.payload); });
}

document.getElementById("peer-ip").addEventListener("keydown", function (e) {
  if (e.key === "Enter") connectPeer();
});

window.addEventListener("resize", function () {
  if (layoutMonitors.length > 0) renderMonitors();
});

document.addEventListener("DOMContentLoaded", init);
