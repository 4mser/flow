use flow_core::clipboard::ClipboardSync;
use flow_core::discovery::{Discovery, DiscoveryEvent};
use flow_core::mesh::MeshNode;
use flow_core::monitor::detect_monitors;
use flow_core::transfer::FileTransferManager;
use flow_protocol::{
    ClipboardContentType, CursorColor, FlowMessage, OsType, PeerId, PeerInfo,
};
use serde::Serialize;
use std::collections::HashSet;
use std::net::UdpSocket;
use std::path::Path;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::RwLock;
use tracing::error;

struct FlowState {
    peer_info: PeerInfo,
    mesh: Arc<MeshNode>,
    transfers: Arc<FileTransferManager>,
    connected_peers: Arc<RwLock<Vec<PeerUi>>>,
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

    let state = FlowState {
        peer_info: peer_info.clone(),
        mesh: mesh.clone(),
        transfers: transfers.clone(),
        connected_peers: connected_peers.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![get_status, connect_peer, get_peers, send_file, pick_and_send, setup_firewall])
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

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running FLOW");
}
