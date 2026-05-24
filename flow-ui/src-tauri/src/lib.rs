use flow_core::clipboard::ClipboardSync;
use flow_core::cursor::capture::MouseCapture;
use flow_core::cursor::CursorManager;
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
use tauri::{Emitter, Manager, WebviewWindowBuilder, WebviewUrl};
use tokio::sync::RwLock;
use tracing::{error, info};

struct FlowState {
    peer_info: PeerInfo,
    mesh: Arc<MeshNode>,
    transfers: Arc<FileTransferManager>,
    connected_peers: Arc<RwLock<Vec<PeerUi>>>,
    all_monitors: Arc<RwLock<Vec<LayoutMonitor>>>,
    cursor_mgr: Arc<CursorManager>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct LayoutMonitor {
    peer_id: String,
    peer_name: String,
    monitor_id: u32,
    name: String,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    mine: bool,
    color: String,
}

#[derive(Debug, Clone, Serialize)]
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

    Ok(StatusUi {
        name: info.name.clone(),
        os: format!("{:?}", info.os),
        color: format!("rgb({},{},{})", info.color.r, info.color.g, info.color.b),
        peer_id: info.id.0.to_string()[..8].to_string(),
        monitors: info
            .monitors
            .iter()
            .map(|m| MonitorUi {
                name: m.name.clone(),
                width: m.width,
                height: m.height,
                x: m.position.x,
                y: m.position.y,
                scale: m.scale_factor,
            })
            .collect(),
        peers,
        local_ip: get_local_ip(),
    })
}

#[tauri::command]
async fn connect_peer(ip: String, state: tauri::State<'_, FlowState>) -> Result<String, String> {
    state
        .mesh
        .connect_to(&ip)
        .await
        .map_err(|e| format!("Connection failed: {e}"))?;

    let announce = FlowMessage::Announce(state.peer_info.clone());
    state
        .mesh
        .broadcast(&announce)
        .await
        .map_err(|e| format!("Announce failed: {e}"))?;

    Ok(format!("Connected to {ip}"))
}

#[tauri::command]
async fn get_peers(state: tauri::State<'_, FlowState>) -> Result<Vec<PeerUi>, String> {
    Ok(state.connected_peers.read().await.clone())
}

#[tauri::command]
async fn get_layout(state: tauri::State<'_, FlowState>) -> Result<Vec<LayoutMonitor>, String> {
    Ok(state.all_monitors.read().await.clone())
}

#[tauri::command]
async fn save_layout(
    monitors: Vec<LayoutMonitor>,
    state: tauri::State<'_, FlowState>,
) -> Result<String, String> {
    // Update stored layout
    *state.all_monitors.write().await = monitors.clone();

    // Find my monitors that were repositioned and broadcast update
    let my_id = state.peer_info.id.0.to_string();
    let my_monitors: Vec<flow_protocol::MonitorInfo> = monitors
        .iter()
        .filter(|m| m.peer_id == my_id[..8])
        .map(|m| flow_protocol::MonitorInfo {
            monitor_id: m.monitor_id,
            name: m.name.clone(),
            width: m.width,
            height: m.height,
            scale_factor: 1.0,
            position: flow_protocol::GridPosition { x: m.x, y: m.y },
        })
        .collect();

    // Rebuild the cursor manager spatial layout with ALL monitors
    {
        let mut spatial = state.cursor_mgr.layout.write().await;
        *spatial = flow_protocol::SpatialLayout::new();
        for m in &monitors {
            spatial.entries.push(flow_protocol::SpatialEntry {
                peer_id: PeerId(m.peer_id.parse().unwrap_or_else(|_| uuid::Uuid::new_v4())),
                monitor_id: m.monitor_id,
                x: m.x,
                y: m.y,
                width: m.width,
                height: m.height,
            });
        }
    }

    let msg = FlowMessage::LayoutUpdate {
        peer_id: state.peer_info.id.clone(),
        monitors: my_monitors,
    };
    state.mesh.broadcast(&msg).await.map_err(|e| format!("{e}"))?;

    Ok("Layout saved and synced".to_string())
}

#[tauri::command]
async fn send_file(path: String, state: tauri::State<'_, FlowState>) -> Result<String, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    state
        .transfers
        .send_file(p, &state.peer_info.id, &state.mesh)
        .await
        .map_err(|e| format!("Send failed: {e}"))?;
    Ok(name)
}

#[tauri::command]
async fn pick_and_send(app: tauri::AppHandle, state: tauri::State<'_, FlowState>) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let path = app.dialog()
        .file()
        .blocking_pick_file()
        .ok_or("No file selected".to_string())?;

    let file_path = path.into_path().map_err(|e| format!("Invalid path: {e}"))?;
    let name = file_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

    state
        .transfers
        .send_file(&file_path, &state.peer_info.id, &state.mesh)
        .await
        .map_err(|e| format!("Send failed: {e}"))?;

    let _ = app.emit("file-sent", &name);
    Ok(name)
}

#[tauri::command]
async fn setup_firewall() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let rules = [
            ("FLOW-TCP-IN", "in", "TCP", "19847"),
            ("FLOW-TCP-OUT", "out", "TCP", "19847"),
            ("FLOW-mDNS", "in", "UDP", "5353"),
        ];
        for (name, dir, proto, port) in &rules {
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
    {
        Ok("Not needed on this OS".to_string())
    }
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

    let my_id_short = peer_info.id.0.to_string()[..8].to_string();
    let initial_layout: Vec<LayoutMonitor> = peer_info.monitors.iter().map(|m| {
        LayoutMonitor {
            peer_id: my_id_short.clone(),
            peer_name: peer_info.name.clone(),
            monitor_id: m.monitor_id,
            name: m.name.clone(),
            width: m.width,
            height: m.height,
            x: m.position.x,
            y: m.position.y,
            mine: true,
            color: format!("rgb({},{},{})", peer_info.color.r, peer_info.color.g, peer_info.color.b),
        }
    }).collect();
    let all_monitors: Arc<RwLock<Vec<LayoutMonitor>>> = Arc::new(RwLock::new(initial_layout));
    let cursor_mgr = Arc::new(CursorManager::new(PeerId(peer_id.0)));
    {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let mut layout = cursor_mgr.layout.write().await;
            layout.add_peer_monitors(&peer_info);
        });
    }

    let state = FlowState {
        peer_info: peer_info.clone(),
        mesh: mesh.clone(),
        transfers: transfers.clone(),
        connected_peers: connected_peers.clone(),
        all_monitors: all_monitors.clone(),
        cursor_mgr: cursor_mgr.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![get_status, connect_peer, get_peers, get_layout, save_layout, send_file, pick_and_send, setup_firewall])
        .setup(move |app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }

            // Auto-configure firewall on Windows
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

            let handle = app.handle().clone();

            let discovery =
                Discovery::new(&peer_name, &peer_id, color).expect("Failed to start discovery");
            let mut discovery_events = discovery.subscribe();
            let mut mesh_messages = mesh.subscribe();
            let mut clipboard_changes = clipboard.subscribe();

            let mesh_listen = mesh.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = mesh_listen.listen().await {
                    error!("Mesh listen error: {e}");
                }
            });

            tauri::async_runtime::spawn(async move {
                if let Err(e) = discovery.run().await {
                    error!("Discovery error: {e}");
                }
            });

            let mesh_connect = mesh.clone();
            let pi = peer_info.clone();
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
                                        let announce = FlowMessage::Announce(pi.clone());
                                        let _ = mesh_connect.broadcast(&announce).await;

                                        let peer_ui = PeerUi {
                                            name: peer.name.clone(),
                                            os: format!("{:?}", peer.os),
                                            color: format!(
                                                "rgb({},{},{})",
                                                peer.color.r, peer.color.g, peer.color.b
                                            ),
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

            let clipboard_remote = clipboard.clone();
            let transfers_msg = transfers.clone();
            let peers_msg = connected_peers.clone();
            let layout_msg = all_monitors.clone();
            let cursor_msg = cursor_mgr.clone();
            let handle_msg = handle.clone();
            let my_id = peer_info.id.0.to_string();
            let my_id_short = my_id[..8].to_string();
            let mesh_respond = mesh.clone();
            let my_info = peer_info.clone();
            let announced = announced_to.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(msg) = mesh_messages.recv().await {
                    match msg {
                        FlowMessage::Announce(peer) => {
                            let pid = peer.id.0.to_string();
                            if pid == my_id {
                                continue;
                            }
                            let peer_ui = PeerUi {
                                name: peer.name.clone(),
                                os: format!("{:?}", peer.os),
                                color: format!(
                                    "rgb({},{},{})",
                                    peer.color.r, peer.color.g, peer.color.b
                                ),
                                peer_id: pid[..8].to_string(),
                            };
                            let mut peers = peers_msg.write().await;
                            if !peers.iter().any(|p| p.peer_id == peer_ui.peer_id) {
                                peers.push(peer_ui.clone());
                                let _ = handle_msg.emit("peer-joined", &peer_ui);
                            }
                            drop(peers);

                            // Add peer monitors to cursor manager spatial layout
                            if !peer.monitors.is_empty() {
                                let mut spatial = cursor_msg.layout.write().await;
                                spatial.add_peer_monitors(&peer);
                                drop(spatial);
                            }

                            // Add peer monitors to UI layout
                            if !peer.monitors.is_empty() {
                                let peer_color = format!("rgb({},{},{})", peer.color.r, peer.color.g, peer.color.b);
                                let mut layout = layout_msg.write().await;
                                let existing_ids: Vec<String> = layout.iter().map(|m| m.peer_id.clone()).collect();
                                if !existing_ids.contains(&pid[..8].to_string()) {
                                    // Position peer monitors to the right of existing monitors
                                    let max_x: i32 = layout.iter().map(|m| m.x + m.width as i32).max().unwrap_or(0);
                                    for (i, m) in peer.monitors.iter().enumerate() {
                                        layout.push(LayoutMonitor {
                                            peer_id: pid[..8].to_string(),
                                            peer_name: peer.name.clone(),
                                            monitor_id: m.monitor_id,
                                            name: m.name.clone(),
                                            width: m.width,
                                            height: m.height,
                                            x: max_x + m.position.x,
                                            y: m.position.y,
                                            mine: false,
                                            color: peer_color.clone(),
                                        });
                                    }
                                }
                                drop(layout);
                                let _ = handle_msg.emit("layout-updated", "");
                            }

                            let mut set = announced.write().await;
                            if !set.contains(&pid) {
                                set.insert(pid);
                                let _ = mesh_respond.broadcast(&FlowMessage::Announce(my_info.clone())).await;
                            }
                        }
                        FlowMessage::ClipboardData { peer_id: pid, data, .. } => {
                            if pid.0.to_string() == my_id {
                                continue;
                            }
                            if let Ok(text) = String::from_utf8(data) {
                                let _ = handle_msg.emit("clipboard-received", &text);
                                clipboard_remote.apply_remote(text).await;
                            }
                        }
                        FlowMessage::FileOffer { transfer_id, file_name, file_size, from_peer } => {
                            if from_peer.0.to_string() == my_id {
                                continue;
                            }
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
                        FlowMessage::CursorMove { peer_id: pid, name, color, position, visible, .. } => {
                            if pid.0.to_string() == my_id { continue; }
                            if visible {
                                if let Some(overlay) = handle_msg.get_webview_window("cursor-overlay") {
                                    let _ = overlay.set_position(tauri::Position::Physical(
                                        tauri::PhysicalPosition::new(position.x as i32, position.y as i32)
                                    ));
                                    let _ = overlay.show();
                                    let _ = overlay.emit("update-cursor-overlay", serde_json::json!({
                                        "name": name,
                                        "color": format!("rgb({},{},{})", color.r, color.g, color.b)
                                    }));
                                }
                            }
                        }
                        FlowMessage::CursorReturn { peer_id: pid } => {
                            if pid.0.to_string() == my_id { continue; }
                            if let Some(overlay) = handle_msg.get_webview_window("cursor-overlay") {
                                let _ = overlay.hide();
                            }
                        }
                        FlowMessage::LayoutUpdate { peer_id: pid, monitors: peer_monitors } => {
                            if pid.0.to_string() == my_id {
                                continue;
                            }
                            let pid_short = pid.0.to_string()[..8].to_string();
                            let mut layout = layout_msg.write().await;
                            layout.retain(|m| m.peer_id != pid_short);
                            for m in &peer_monitors {
                                layout.push(LayoutMonitor {
                                    peer_id: pid_short.clone(),
                                    peer_name: "Peer".to_string(),
                                    monitor_id: m.monitor_id,
                                    name: m.name.clone(),
                                    width: m.width,
                                    height: m.height,
                                    x: m.position.x,
                                    y: m.position.y,
                                    mine: false,
                                    color: "rgb(59,130,246)".to_string(),
                                });
                            }
                            drop(layout);
                            let _ = handle_msg.emit("layout-updated", "");
                        }
                        _ => {}
                    }
                }
            });

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

            // --- Cursor sharing ---
            // Set up overlay window as click-through
            if let Some(overlay_win) = app.get_webview_window("cursor-overlay") {
                let _ = overlay_win.set_ignore_cursor_events(true);
            }

            // Start mouse capture
            let mouse_capture = Arc::new(MouseCapture::new());
            mouse_capture.start_polling();

            // Edge detection: when cursor hits screen edge, send to peer
            let mesh_cursor = mesh.clone();
            let cursor_edge = cursor_mgr.clone();
            let mut mouse_rx = mouse_capture.subscribe();
            let cursor_name = peer_info.name.clone();
            let cursor_color = peer_info.color;
            let cursor_pid = PeerId(peer_info.id.0);
            let handle_cursor = handle.clone();
            tauri::async_runtime::spawn(async move {
                let mut is_remote = false;
                let mut frame_skip: u32 = 0;
                while let Ok(pos) = mouse_rx.recv().await {
                    // Only check every 3rd frame to reduce CPU
                    frame_skip += 1;
                    if frame_skip % 3 != 0 { continue; }

                    if let Some(event) = cursor_edge.check_edge(
                        pos.x, pos.y, pos.screen_width, pos.screen_height, pos.monitor_id
                    ).await {
                        match event {
                            flow_core::cursor::CursorEvent::CrossedEdge { to_peer_id: _, to_monitor, position } => {
                                if !is_remote {
                                    is_remote = true;
                                    info!("Cursor crossed to remote monitor");
                                }
                                let msg = FlowMessage::CursorMove {
                                    peer_id: cursor_pid.clone(),
                                    name: cursor_name.clone(),
                                    color: cursor_color,
                                    monitor_id: to_monitor,
                                    position,
                                    visible: true,
                                };
                                let _ = mesh_cursor.broadcast(&msg).await;
                            }
                            _ => {}
                        }
                    } else if is_remote {
                        is_remote = false;
                        let msg = FlowMessage::CursorReturn { peer_id: cursor_pid.clone() };
                        let _ = mesh_cursor.broadcast(&msg).await;
                    }
                }
            });

            // Handle remote cursor: move overlay window to cursor position
            // (CursorMove messages are handled in the message handler above,
            //  but we need to add overlay logic there too)

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running FLOW");
}
