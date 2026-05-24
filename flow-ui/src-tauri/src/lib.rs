use flow_core::clipboard::ClipboardSync;
use flow_core::discovery::{Discovery, DiscoveryEvent};
use flow_core::mesh::MeshNode;
use flow_core::monitor::detect_monitors;
use flow_protocol::{
    ClipboardContentType, CursorColor, FlowMessage, OsType, PeerId, PeerInfo,
};
use serde::Serialize;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::RwLock;
use tracing::error;

struct FlowState {
    peer_info: PeerInfo,
    mesh: Arc<MeshNode>,
    clipboard: Arc<ClipboardSync>,
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
    let connected_peers: Arc<RwLock<Vec<PeerUi>>> = Arc::new(RwLock::new(Vec::new()));

    let state = FlowState {
        peer_info: peer_info.clone(),
        mesh: mesh.clone(),
        clipboard: clipboard.clone(),
        connected_peers: connected_peers.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![get_status, connect_peer, get_peers])
        .setup(move |app| {
            let handle = app.handle().clone();

            let discovery =
                Discovery::new(&peer_name, &peer_id, color).expect("Failed to start discovery");
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

            // Discovery → connect + emit to frontend
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
                                    if let Ok(_) = mesh_connect.connect_to(&ip).await {
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

            // Incoming messages
            let clipboard_remote = clipboard.clone();
            let peers_msg = connected_peers.clone();
            let handle_msg = handle.clone();
            let my_id = peer_info.id.0.to_string();
            tauri::async_runtime::spawn(async move {
                while let Ok(msg) = mesh_messages.recv().await {
                    match msg {
                        FlowMessage::Announce(peer) => {
                            let peer_ui = PeerUi {
                                name: peer.name.clone(),
                                os: format!("{:?}", peer.os),
                                color: format!(
                                    "rgb({},{},{})",
                                    peer.color.r, peer.color.g, peer.color.b
                                ),
                                peer_id: peer.id.0.to_string()[..8].to_string(),
                            };
                            let mut peers = peers_msg.write().await;
                            if !peers.iter().any(|p| p.peer_id == peer_ui.peer_id) {
                                peers.push(peer_ui.clone());
                                let _ = handle_msg.emit("peer-joined", &peer_ui);
                            }
                        }
                        FlowMessage::ClipboardData { peer_id, data, .. } => {
                            if peer_id.0.to_string() == my_id {
                                continue;
                            }
                            if let Ok(text) = String::from_utf8(data) {
                                let _ = handle_msg.emit("clipboard-received", &text);
                                clipboard_remote.apply_remote(text).await;
                            }
                        }
                        _ => {}
                    }
                }
            });

            // Clipboard watcher → broadcast
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
