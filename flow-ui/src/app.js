const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let status = null;
let currentDirection = "right";

async function init() {
  try {
    status = await invoke("get_status");
    currentDirection = status.peer_direction || "right";
    renderStatus();
    updateDirectionUI();
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

async function setDirection(dir) {
  currentDirection = dir;
  await invoke("set_peer_direction", { direction: dir });
  updateDirectionUI();
}

function updateDirectionUI() {
  document.querySelectorAll(".dir-btn[data-dir]").forEach(function (btn) {
    btn.classList.toggle("active", btn.dataset.dir === currentDirection);
  });
  var labels = {
    right: "Cursor will cross to peer from the right edge",
    left: "Cursor will cross to peer from the left edge",
    above: "Cursor will cross to peer from the top edge",
    below: "Cursor will cross to peer from the bottom edge",
  };
  document.getElementById("direction-status").textContent = labels[currentDirection] || "";
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
  statusEl.textContent = "Selecting...";
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
    var fs = document.getElementById("file-status");
    if (fs) fs.textContent = "Received: " + e.payload;
  });
  await listen("file-sent", function (e) { addClip("sent", "File sent: " + e.payload); });
  await listen("file-complete", function (e) { addClip("received", "Transfer done: " + e.payload); });

  await listen("scan-started", function (e) {
    document.getElementById("connect-hint").textContent = "Scanning " + e.payload + ".0/24...";
  });
  await listen("scan-complete", function () {
    document.getElementById("connect-hint").textContent = "Or enter IP manually";
  });
  await listen("peer-found-scan", function (e) {
    addClip("received", "Found peer at " + e.payload);
    document.getElementById("connect-hint").textContent = "Auto-connected to " + e.payload;
  });
}

document.getElementById("peer-ip").addEventListener("keydown", function (e) {
  if (e.key === "Enter") connectPeer();
});

document.addEventListener("DOMContentLoaded", init);
