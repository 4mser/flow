use clap::Parser;
use flow_core::clipboard::ClipboardSync;
use flow_core::cursor::capture::MouseCapture;
use flow_core::cursor::overlay::CursorOverlay;
use flow_core::cursor::CursorManager;
use flow_core::discovery::{Discovery, DiscoveryEvent};
use flow_core::mesh::MeshNode;
use flow_core::monitor::detect_monitors;
use flow_core::transfer::FileTransferManager;
use flow_protocol::{
    ClipboardContentType, CursorColor, CursorPosition, FlowMessage, OsType, PeerId, PeerInfo,
    SpatialLayout,
};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "flow", about = "FLOW — Multiplayer Operating System")]
struct Cli {
    #[arg(short, long, default_value = "")]
    name: String,

    #[arg(short, long, default_value_t = 0)]
    color: usize,

    #[arg(short, long)]
    peer: Option<String>,

    #[arg(short, long)]
    send: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "flow=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    let peer_id = PeerId::new();
    let peer_name = if cli.name.is_empty() {
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unnamed".to_string())
    } else {
        cli.name.clone()
    };
    let color = CursorColor::pick(cli.color);
    let monitors = detect_monitors();

    println!();
    println!("  ╔═══════════════════════════════════════╗");
    println!("  ║        FLOW — Multiplayer OS          ║");
    println!("  ╚═══════════════════════════════════════╝");
    println!();
    println!("  Name:    {peer_name}");
    println!("  OS:      {:?}", OsType::current());
    println!("  Color:   rgb({}, {}, {})", color.r, color.g, color.b);
    println!("  Peer ID: {}", &peer_id.0.to_string()[..8]);
    println!();

    for m in &monitors {
        println!(
            "  Monitor: {} — {}x{} @ ({}, {}) scale={:.1}",
            m.name, m.width, m.height, m.position.x, m.position.y, m.scale_factor
        );
    }
    println!();

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
    let announced_to: Arc<RwLock<HashSet<String>>> = Arc::new(RwLock::new(HashSet::new()));

    let discovery = Discovery::new(&peer_name, &peer_id, color)?;
    let mut discovery_events = discovery.subscribe();
    let mut mesh_messages = mesh.subscribe();
    let mut clipboard_changes = clipboard.subscribe();

    // Listen for incoming TCP connections
    let mesh_listen = mesh.clone();
    tokio::spawn(async move {
        if let Err(e) = mesh_listen.listen().await {
            error!("Mesh listen error: {e}");
            std::process::exit(1);
        }
    });

    // Direct connection with retry
    if let Some(ref peer_ip) = cli.peer {
        let mesh_direct = mesh.clone();
        let ip = peer_ip.clone();
        let pi = peer_info.clone();
        tokio::spawn(async move {
            let mut delay = 2;
            loop {
                println!("  Connecting to {ip}...");
                match mesh_direct.connect_to(&ip).await {
                    Ok(_) => {
                        let announce = FlowMessage::Announce(pi);
                        let _ = mesh_direct.broadcast(&announce).await;
                        println!("  === Connected to {ip} ===");
                        break;
                    }
                    Err(e) => {
                        warn!("Connection to {ip} failed: {e}, retrying in {delay}s...");
                        println!("  Connection failed, retrying in {delay}s...");
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        delay = (delay * 2).min(30);
                    }
                }
            }
        });
    }

    // mDNS discovery
    tokio::spawn(async move {
        if let Err(e) = discovery.run().await {
            error!("Discovery error: {e}");
        }
    });

    // Auto-connect on peer discovery
    let mesh_connect = mesh.clone();
    let my_peer_info = peer_info.clone();
    tokio::spawn(async move {
        while let Ok(event) = discovery_events.recv().await {
            match event {
                DiscoveryEvent::PeerFound { peer, addresses } => {
                    println!(
                        "  >>> Peer discovered: {} ({:?}) at {:?}",
                        peer.name, peer.os, addresses
                    );
                    for addr in &addresses {
                        if addr.is_ipv4() {
                            let ip = addr.to_string();
                            if let Ok(_) = mesh_connect.connect_to(&ip).await {
                                let announce = FlowMessage::Announce(my_peer_info.clone());
                                let _ = mesh_connect.broadcast(&announce).await;
                                println!("  === Connected and syncing with {} ===", peer.name);
                            }
                            break;
                        }
                    }
                }
                DiscoveryEvent::PeerLost(id) => {
                    println!("  <<< Peer left: {id}");
                }
            }
        }
    });

    // Cursor overlay (renders remote cursors on our screen)
    let overlay = Arc::new(CursorOverlay::new());
    overlay.start_render_loop();

    // Cursor manager (spatial layout + edge detection)
    let cursor_mgr = Arc::new(CursorManager::new(PeerId(peer_id.0)));
    {
        let mut layout = cursor_mgr.layout.write().await;
        layout.add_peer_monitors(&peer_info);
    }

    // Mouse capture (polls cursor position at 60fps)
    let mouse_capture = Arc::new(MouseCapture::new());
    mouse_capture.start_polling();

    // Handle incoming messages
    let overlay_ref = overlay.clone();
    let clipboard_remote = clipboard.clone();
    let transfers_recv = transfers.clone();
    let mesh_respond = mesh.clone();
    let my_info_for_announce = peer_info.clone();
    let my_id = peer_id.0.to_string();
    let announced = announced_to.clone();
    let cursor_mgr_msg = cursor_mgr.clone();
    tokio::spawn(async move {
        while let Ok(msg) = mesh_messages.recv().await {
            match msg {
                FlowMessage::Announce(peer) => {
                    let pid = peer.id.0.to_string();
                    if pid == my_id {
                        continue;
                    }
                    println!("  Connected: {} ({:?}) — {} monitors", peer.name, peer.os, peer.monitors.len());

                    // Add peer monitors to spatial layout
                    if !peer.monitors.is_empty() {
                        let mut layout = cursor_mgr_msg.layout.write().await;
                        layout.add_peer_monitors(&peer);
                    }

                    // Respond with our announce if we haven't already
                    let mut set = announced.write().await;
                    if !set.contains(&pid) {
                        set.insert(pid);
                        let _ = mesh_respond.broadcast(&FlowMessage::Announce(my_info_for_announce.clone())).await;
                    }
                }
                FlowMessage::ClipboardData { peer_id: pid, data, .. } => {
                    if pid.0.to_string() == my_id {
                        continue;
                    }
                    if let Ok(text) = String::from_utf8(data) {
                        println!("  [clipboard] ← {}...", &text[..text.len().min(60)]);
                        clipboard_remote.apply_remote(text).await;
                    }
                }
                FlowMessage::FileOffer { transfer_id, file_name, file_size, from_peer } => {
                    if from_peer.0.to_string() == my_id {
                        continue;
                    }
                    println!("  [file] ← Receiving: {file_name} ({:.1} MB)", file_size as f64 / 1024.0 / 1024.0);
                    if let Err(e) = transfers_recv.handle_offer(transfer_id, &file_name, file_size).await {
                        error!("Failed to start receiving file: {e}");
                    }
                }
                FlowMessage::FileChunk { transfer_id, offset, data, is_last } => {
                    match transfers_recv.handle_chunk(transfer_id, offset, &data, is_last).await {
                        Ok(Some(name)) => {
                            println!("  [file] ✓ Received: {name} → {}", transfers_recv.download_dir().display());
                        }
                        Ok(None) => {}
                        Err(e) => error!("File chunk error: {e}"),
                    }
                }
                FlowMessage::FileComplete { file_name, .. } => {
                    println!("  [file] Transfer complete: {file_name}");
                }
                FlowMessage::CursorMove { peer_id: pid, name, color, position, visible, .. } => {
                    if pid.0.to_string() == my_id {
                        continue;
                    }
                    overlay_ref.update(position.x, position.y, color, &name, visible);
                }
                FlowMessage::CursorReturn { peer_id: pid } => {
                    if pid.0.to_string() == my_id {
                        continue;
                    }
                    overlay_ref.hide();
                }
                _ => {}
            }
        }
    });

    // Clipboard broadcast
    let mesh_clipboard = mesh.clone();
    let clip_id = PeerId(peer_id.0);
    tokio::spawn(async move {
        while let Ok(text) = clipboard_changes.recv().await {
            println!("  [clipboard] → {}...", &text[..text.len().min(60)]);
            let msg = FlowMessage::ClipboardData {
                peer_id: clip_id.clone(),
                content_type: ClipboardContentType::PlainText,
                data: text.into_bytes(),
            };
            if let Err(e) = mesh_clipboard.broadcast(&msg).await {
                error!("Failed to broadcast clipboard: {e}");
            }
        }
    });

    // Watch clipboard
    let clipboard_watcher = clipboard.clone();
    tokio::spawn(async move {
        clipboard_watcher.watch().await;
    });

    // Send file if --send was provided
    if let Some(ref file_path) = cli.send {
        let transfers_send = transfers.clone();
        let mesh_send = mesh.clone();
        let pid = PeerId(peer_id.0);
        let fp = file_path.clone();
        tokio::spawn(async move {
            // Wait for connection
            tokio::time::sleep(Duration::from_secs(3)).await;
            if mesh_send.peer_count().await == 0 {
                println!("  [file] Waiting for a peer to connect...");
                while mesh_send.peer_count().await == 0 {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
            println!("  [file] → Sending: {fp}");
            match transfers_send.send_file(Path::new(&fp), &pid, &mesh_send).await {
                Ok(_) => println!("  [file] ✓ File sent successfully"),
                Err(e) => error!("Failed to send file: {e}"),
            }
        });
    }

    // Cursor edge detection → send position to peers
    let mesh_cursor = mesh.clone();
    let cursor_mgr_edge = cursor_mgr.clone();
    let mut mouse_events = mouse_capture.subscribe_pos();
    let cursor_peer_name = peer_info.name.clone();
    let cursor_peer_color = peer_info.color;
    let cursor_peer_id = PeerId(peer_id.0);
    let mouse_captured = mouse_capture.captured.clone();
    tokio::spawn(async move {
        let mut is_on_remote = false;
        while let Ok(pos) = mouse_events.recv().await {
            if let Some(event) = cursor_mgr_edge.check_edge(
                pos.x, pos.y, pos.screen_width, pos.screen_height, pos.monitor_id
            ).await {
                match event {
                    flow_core::cursor::CursorEvent::CrossedEdge { to_peer_id: _, to_monitor, position } => {
                        if !is_on_remote {
                            is_on_remote = true;
                            println!("  [cursor] → Cursor crossed to remote monitor");
                        }
                        let msg = FlowMessage::CursorMove {
                            peer_id: cursor_peer_id.clone(),
                            name: cursor_peer_name.clone(),
                            color: cursor_peer_color,
                            monitor_id: to_monitor,
                            position,
                            visible: true,
                        };
                        let _ = mesh_cursor.broadcast(&msg).await;
                    }
                    _ => {}
                }
            } else if is_on_remote {
                is_on_remote = false;
                let msg = FlowMessage::CursorReturn {
                    peer_id: cursor_peer_id.clone(),
                };
                let _ = mesh_cursor.broadcast(&msg).await;
            }
        }
    });

    println!("  Cursor sharing active.");
    println!("  Clipboard sync active. File transfer ready.");
    if cli.peer.is_none() {
        println!("  Searching for peers on the network...");
        println!("  Tip: use --peer <IP> for direct connection");
    }
    println!("  Press Ctrl+C to stop.");
    println!();

    tokio::signal::ctrl_c().await?;
    println!("\n  FLOW agent stopped.");

    Ok(())
}
