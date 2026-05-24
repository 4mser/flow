use flow_core::clipboard::ClipboardSync;
use flow_core::cursor::capture::MouseCapture;
use flow_core::discovery::{Discovery, DiscoveryEvent};
use flow_core::mesh::MeshNode;
use flow_core::monitor::detect_monitors;
use flow_core::transfer::FileTransferManager;
use flow_protocol::{
    ClipboardContentType, CursorColor, CursorPosition, FlowMessage, OsType, PeerId, PeerInfo,
};
use serde::Serialize;
use std::collections::HashSet;
use std::net::UdpSocket;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
use tokio::sync::RwLock;
use tracing::{error, info};

struct FlowState {
    peer_info: PeerInfo,
    mesh: Arc<MeshNode>,
    transfers: Arc<FileTransferManager>,
    connected_peers: Arc<RwLock<Vec<PeerUi>>>,
    peer_direction: Arc<RwLock<String>>, // "right", "left", "above", "below"
    cursor_on_remote: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct PeerUi {
    name: String,
    os: String,
    color: String,
    peer_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct MonitorUi {
    name: String,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    scale: f64,
}

#[derive(Debug, Clone, Serialize)]
struct StatusUi {
    name: String,
    os: String,
    color: String,
    peer_id: String,
    monitors: Vec<MonitorUi>,
    peers: Vec<PeerUi>,
    local_ip: String,
    peer_direction: String,
}

fn get_local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

#[tauri::command]
async fn get_status(state: tauri::State<'_, FlowState>) -> Result<StatusUi, String> {
    let peers = state.connected_peers.read().await.clone();
    let info = &state.peer_info;
    let dir = state.peer_direction.read().await.clone();

    Ok(StatusUi {
        name: info.name.clone(),
        os: format!("{:?}", info.os),
        color: format!("rgb({},{},{})", info.color.r, info.color.g, info.color.b),
        peer_id: info.id.0.to_string()[..8].to_string(),
        monitors: info.monitors.iter().map(|m| MonitorUi {
            name: m.name.clone(),
            width: m.width,
            height: m.height,
            x: m.position.x,
            y: m.position.y,
            scale: m.scale_factor,
        }).collect(),
        peers,
        local_ip: get_local_ip(),
        peer_direction: dir,
    })
}

#[tauri::command]
async fn connect_peer(ip: String, state: tauri::State<'_, FlowState>) -> Result<String, String> {
    state.mesh.connect_to(&ip).await.map_err(|e| format!("Connection failed: {e}"))?;
    let announce = FlowMessage::Announce(state.peer_info.clone());
    state.mesh.broadcast(&announce).await.map_err(|e| format!("Announce failed: {e}"))?;
    Ok(format!("Connected to {ip}"))
}

#[tauri::command]
async fn get_peers(state: tauri::State<'_, FlowState>) -> Result<Vec<PeerUi>, String> {
    Ok(state.connected_peers.read().await.clone())
}

#[tauri::command]
async fn set_peer_direction(direction: String, state: tauri::State<'_, FlowState>) -> Result<String, String> {
    *state.peer_direction.write().await = direction.clone();
    info!("Peer direction set to: {direction}");

    let msg = FlowMessage::DirectionSet {
        peer_id: state.peer_info.id.clone(),
        direction: direction.clone(),
    };
    state.mesh.broadcast(&msg).await.map_err(|e| format!("{e}"))?;

    Ok(direction)
}

fn invert_direction(dir: &str) -> &str {
    match dir {
        "right" => "left",
        "left" => "right",
        "above" => "below",
        "below" => "above",
        _ => "right",
    }
}

#[tauri::command]
async fn send_file(path: String, state: tauri::State<'_, FlowState>) -> Result<String, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    state.transfers.send_file(p, &state.peer_info.id, &state.mesh).await
        .map_err(|e| format!("Send failed: {e}"))?;
    Ok(name)
}

#[tauri::command]
async fn pick_and_send(app: tauri::AppHandle, state: tauri::State<'_, FlowState>) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog().file().blocking_pick_file()
        .ok_or("No file selected".to_string())?;
    let file_path = path.into_path().map_err(|e| format!("Invalid path: {e}"))?;
    let name = file_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    state.transfers.send_file(&file_path, &state.peer_info.id, &state.mesh).await
        .map_err(|e| format!("Send failed: {e}"))?;
    let _ = app.emit("file-sent", &name);
    Ok(name)
}

#[tauri::command]
async fn setup_firewall() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        for (name, dir, proto, port) in [
            ("FLOW-TCP-IN", "in", "TCP", "19847"),
            ("FLOW-TCP-OUT", "out", "TCP", "19847"),
            ("FLOW-mDNS", "in", "UDP", "5353"),
        ] {
            let _ = std::process::Command::new("netsh")
                .args(["advfirewall", "firewall", "add", "rule",
                    &format!("name={name}"), &format!("dir={dir}"),
                    "action=allow", &format!("protocol={proto}"),
                    &format!("localport={port}")])
                .output();
        }
        Ok("Firewall rules configured".to_string())
    }
    #[cfg(not(target_os = "windows"))]
    { Ok("Not needed on this OS".to_string()) }
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "flow=info".parse().unwrap()),
        )
        .init();

    let peer_id = PeerId::new();
    let peer_name = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unnamed".to_string());
    let color = CursorColor::pick(0);
    let monitors = detect_monitors();

    let peer_info = PeerInfo {
        id: peer_id.clone(),
        name: peer_name.clone(),
        color,
        monitors,
        os: OsType::current(),
    };

    let mesh = Arc::new(MeshNode::new());
    let clipboard = Arc::new(ClipboardSync::new());
    let transfers = Arc::new(FileTransferManager::new());
    let connected_peers: Arc<RwLock<Vec<PeerUi>>> = Arc::new(RwLock::new(Vec::new()));
    let announced_to: Arc<RwLock<HashSet<String>>> = Arc::new(RwLock::new(HashSet::new()));
    let peer_direction: Arc<RwLock<String>> = Arc::new(RwLock::new("right".to_string()));
    let cursor_on_remote = Arc::new(AtomicBool::new(false));

    let state = FlowState {
        peer_info: peer_info.clone(),
        mesh: mesh.clone(),
        transfers: transfers.clone(),
        connected_peers: connected_peers.clone(),
        peer_direction: peer_direction.clone(),
        cursor_on_remote: cursor_on_remote.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_status, connect_peer, get_peers, set_peer_direction,
            send_file, pick_and_send, setup_firewall
        ])
        .setup(move |app| {
            #[cfg(debug_assertions)]
            {
                if let Some(w) = app.get_webview_window("main") {
                    w.open_devtools();
                }
            }

            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("netsh")
                    .args(["advfirewall", "firewall", "add", "rule", "name=FLOW-TCP-IN", "dir=in", "action=allow", "protocol=TCP", "localport=19847"])
                    .output();
                let _ = std::process::Command::new("netsh")
                    .args(["advfirewall", "firewall", "add", "rule", "name=FLOW-TCP-OUT", "dir=out", "action=allow", "protocol=TCP", "localport=19847"])
                    .output();
                let _ = std::process::Command::new("netsh")
                    .args(["advfirewall", "firewall", "add", "rule", "name=FLOW-mDNS", "dir=in", "action=allow", "protocol=UDP", "localport=5353"])
                    .output();
            }

            // Set up overlay window
            if let Some(overlay) = app.get_webview_window("cursor-overlay") {
                let _ = overlay.set_ignore_cursor_events(true);
            }

            let handle = app.handle().clone();

            let discovery = Discovery::new(&peer_name, &peer_id, color).expect("Failed to start discovery");
            let mut discovery_events = discovery.subscribe();
            let mut mesh_messages = mesh.subscribe();
            let mut clipboard_changes = clipboard.subscribe();

            // Mesh listener
            let mesh_listen = mesh.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = mesh_listen.listen().await {
                    error!("Mesh listen error: {e}");
                }
            });

            // mDNS discovery
            tauri::async_runtime::spawn(async move {
                if let Err(e) = discovery.run().await {
                    error!("Discovery error: {e}");
                }
            });

            // Network scan
            let mesh_scan = mesh.clone();
            let pi_scan = peer_info.clone();
            let handle_scan = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let local_ip = get_local_ip();
                if local_ip == "unknown" { return; }
                let parts: Vec<&str> = local_ip.split('.').collect();
                if parts.len() != 4 { return; }
                let subnet = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
                let my_last: u8 = parts[3].parse().unwrap_or(0);
                info!("Scanning {subnet}.0/24...");
                let _ = handle_scan.emit("scan-started", &subnet);

                let mut handles = Vec::new();
                for i in 1u8..=254 {
                    if i == my_last { continue; }
                    let ip = format!("{subnet}.{i}");
                    handles.push(tokio::spawn(async move {
                        match tokio::time::timeout(
                            std::time::Duration::from_millis(300),
                            tokio::net::TcpStream::connect(format!("{ip}:19847")),
                        ).await {
                            Ok(Ok(_)) => Some(ip),
                            _ => None,
                        }
                    }));
                }
                for h in handles {
                    if let Ok(Some(ip)) = h.await {
                        info!("Found FLOW peer at {ip}");
                        if mesh_scan.connect_to(&ip).await.is_ok() {
                            let _ = mesh_scan.broadcast(&FlowMessage::Announce(pi_scan.clone())).await;
                            let _ = handle_scan.emit("peer-found-scan", &ip);
                        }
                    }
                }
                let _ = handle_scan.emit("scan-complete", "");
            });

            // Discovery → connect
            let mesh_connect = mesh.clone();
            let pi_disc = peer_info.clone();
            let peers_disc = connected_peers.clone();
            let handle_disc = handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = discovery_events.recv().await {
                    match event {
                        DiscoveryEvent::PeerFound { peer, addresses } => {
                            for addr in &addresses {
                                if addr.is_ipv4() {
                                    let ip = addr.to_string();
                                    if mesh_connect.connect_to(&ip).await.is_ok() {
                                        let _ = mesh_connect.broadcast(&FlowMessage::Announce(pi_disc.clone())).await;
                                        let peer_ui = PeerUi {
                                            name: peer.name.clone(),
                                            os: format!("{:?}", peer.os),
                                            color: format!("rgb({},{},{})", peer.color.r, peer.color.g, peer.color.b),
                                            peer_id: peer.id.0.to_string()[..8].to_string(),
                                        };
                                        peers_disc.write().await.push(peer_ui.clone());
                                        let _ = handle_disc.emit("peer-joined", &peer_ui);
                                    }
                                    break;
                                }
                            }
                        }
                        DiscoveryEvent::PeerLost(id) => {
                            let _ = handle_disc.emit("peer-left", &id);
                        }
                    }
                }
            });

            // Message handler
            let clipboard_remote = clipboard.clone();
            let transfers_msg = transfers.clone();
            let peers_msg = connected_peers.clone();
            let peer_direction_msg = peer_direction.clone();
            let handle_msg = handle.clone();
            let my_id = peer_info.id.0.to_string();
            let mesh_respond = mesh.clone();
            let my_info = peer_info.clone();
            let announced = announced_to.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(msg) = mesh_messages.recv().await {
                    match msg {
                        FlowMessage::Announce(peer) => {
                            let pid = peer.id.0.to_string();
                            if pid == my_id { continue; }
                            let peer_ui = PeerUi {
                                name: peer.name.clone(),
                                os: format!("{:?}", peer.os),
                                color: format!("rgb({},{},{})", peer.color.r, peer.color.g, peer.color.b),
                                peer_id: pid[..8].to_string(),
                            };
                            let mut peers = peers_msg.write().await;
                            if !peers.iter().any(|p| p.peer_id == peer_ui.peer_id) {
                                peers.push(peer_ui.clone());
                                let _ = handle_msg.emit("peer-joined", &peer_ui);
                            }
                            drop(peers);
                            let mut set = announced.write().await;
                            if !set.contains(&pid) {
                                set.insert(pid);
                                let _ = mesh_respond.broadcast(&FlowMessage::Announce(my_info.clone())).await;
                            }
                        }
                        FlowMessage::ClipboardData { peer_id: pid, data, .. } => {
                            if pid.0.to_string() == my_id { continue; }
                            if let Ok(text) = String::from_utf8(data) {
                                let _ = handle_msg.emit("clipboard-received", &text);
                                clipboard_remote.apply_remote(text).await;
                            }
                        }
                        FlowMessage::CursorMove { peer_id: pid, name, color, position, visible, .. } => {
                            if pid.0.to_string() == my_id { continue; }
                            if let Some(overlay) = handle_msg.get_webview_window("cursor-overlay") {
                                if visible {
                                    let _ = overlay.set_position(tauri::Position::Physical(
                                        tauri::PhysicalPosition::new(position.x as i32, position.y as i32)
                                    ));
                                    let _ = overlay.show();
                                    let _ = overlay.emit("update-cursor-overlay", serde_json::json!({
                                        "name": name,
                                        "color": format!("rgb({},{},{})", color.r, color.g, color.b)
                                    }));
                                } else {
                                    let _ = overlay.hide();
                                }
                            }
                        }
                        FlowMessage::CursorReturn { peer_id: pid } => {
                            if pid.0.to_string() == my_id { continue; }
                            if let Some(overlay) = handle_msg.get_webview_window("cursor-overlay") {
                                let _ = overlay.hide();
                            }
                        }
                        FlowMessage::DirectionSet { peer_id: pid, direction } => {
                            if pid.0.to_string() == my_id { continue; }
                            let inv = invert_direction(&direction).to_string();
                            info!("Peer set direction to {direction}, auto-setting ours to {inv}");
                            *peer_direction_msg.write().await = inv.clone();
                            let _ = handle_msg.emit("direction-changed", &inv);
                        }
                        FlowMessage::FileOffer { transfer_id, file_name, file_size, from_peer } => {
                            if from_peer.0.to_string() == my_id { continue; }
                            let _ = handle_msg.emit("file-incoming", &format!("{file_name} ({:.1} MB)", file_size as f64 / 1048576.0));
                            let _ = transfers_msg.handle_offer(transfer_id, &file_name, file_size).await;
                        }
                        FlowMessage::FileChunk { transfer_id, offset, data, is_last } => {
                            if let Ok(Some(name)) = transfers_msg.handle_chunk(transfer_id, offset, &data, is_last).await {
                                let _ = handle_msg.emit("file-received", &name);
                            }
                        }
                        FlowMessage::FileComplete { file_name, .. } => {
                            let _ = handle_msg.emit("file-complete", &file_name);
                        }
                        _ => {}
                    }
                }
            });

            // Clipboard watcher
            let mesh_clip = mesh.clone();
            let clip_id = PeerId(peer_info.id.0);
            let handle_clip = handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(text) = clipboard_changes.recv().await {
                    let _ = handle_clip.emit("clipboard-sent", &text);
                    let msg = FlowMessage::ClipboardData {
                        peer_id: clip_id.clone(),
                        content_type: ClipboardContentType::PlainText,
                        data: text.into_bytes(),
                    };
                    let _ = mesh_clip.broadcast(&msg).await;
                }
            });

            let clipboard_w = clipboard.clone();
            tauri::async_runtime::spawn(async move {
                clipboard_w.watch().await;
            });

            // --- CURSOR SHARING ---
            let mouse_capture = Arc::new(MouseCapture::new());
            mouse_capture.start_polling();

            // Edge detection: enter capture mode when cursor hits the configured edge
            let capture_edge = mouse_capture.clone();
            let mut pos_rx = mouse_capture.subscribe_pos();
            let dir_edge = peer_direction.clone();
            let on_remote_edge = cursor_on_remote.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(pos) = pos_rx.recv().await {
                    if on_remote_edge.load(Ordering::Relaxed) { continue; }

                    let dir = dir_edge.read().await.clone();
                    let margin = 3.0;
                    let sw = pos.screen_width as f64;
                    let sh = pos.screen_height as f64;

                    let at_edge = match dir.as_str() {
                        "right" => pos.x >= sw - margin,
                        "left" => pos.x <= margin,
                        "above" => pos.y <= margin,
                        "below" => pos.y >= sh - margin,
                        _ => false,
                    };

                    if at_edge {
                        on_remote_edge.store(true, Ordering::Relaxed);
                        capture_edge.enter_capture();
                    }
                }
            });

            // Delta tracking: when captured, send deltas as cursor movement
            let mesh_cursor = mesh.clone();
            let capture_delta = mouse_capture.clone();
            let mut delta_rx = mouse_capture.subscribe_delta();
            let cursor_name = peer_info.name.clone();
            let cursor_color = peer_info.color;
            let cursor_pid = PeerId(peer_info.id.0);
            let dir_delta = peer_direction.clone();
            let on_remote_delta = cursor_on_remote.clone();
            tauri::async_runtime::spawn(async move {
                let mut vx: f64 = 0.0;
                let mut vy: f64 = 0.0;
                let remote_w: f64 = 1920.0;
                let remote_h: f64 = 1080.0;

                while let Ok(delta) = delta_rx.recv().await {
                    vx += delta.dx;
                    vy += delta.dy;

                    // Clamp to reasonable bounds
                    vy = vy.clamp(0.0, remote_h);

                    let dir = dir_delta.read().await.clone();

                    // Check if cursor returned to our side
                    let returned = match dir.as_str() {
                        "right" => vx < 0.0,
                        "left" => vx > remote_w,
                        "above" => vy > remote_h,
                        "below" => vy < 0.0,
                        _ => false,
                    };

                    if returned {
                        on_remote_delta.store(false, Ordering::Relaxed);
                        capture_delta.exit_capture();
                        vx = 0.0;
                        vy = 0.0;
                        let msg = FlowMessage::CursorReturn { peer_id: cursor_pid.clone() };
                        let _ = mesh_cursor.broadcast(&msg).await;
                        continue;
                    }

                    // Clamp x to remote screen
                    vx = vx.clamp(0.0, remote_w);

                    let msg = FlowMessage::CursorMove {
                        peer_id: cursor_pid.clone(),
                        name: cursor_name.clone(),
                        color: cursor_color,
                        monitor_id: 0,
                        position: CursorPosition { x: vx, y: vy },
                        visible: true,
                    };
                    let _ = mesh_cursor.broadcast(&msg).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running FLOW");
}
