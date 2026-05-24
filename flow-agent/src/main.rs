use clap::Parser;
use flow_core::clipboard::ClipboardSync;
use flow_core::discovery::{Discovery, DiscoveryEvent};
use flow_core::mesh::MeshNode;
use flow_core::monitor::detect_monitors;
use flow_protocol::{
    ClipboardContentType, CursorColor, FlowMessage, OsType, PeerId, PeerInfo,
};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "flow", about = "FLOW — Multiplayer Operating System")]
struct Cli {
    #[arg(short, long, default_value = "")]
    name: String,

    #[arg(short, long, default_value_t = 0)]
    color: usize,
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
    let discovery = Discovery::new(&peer_name, &peer_id, color)?;
    let mut discovery_events = discovery.subscribe();
    let mut mesh_messages = mesh.subscribe();
    let mut clipboard_changes = clipboard.subscribe();

    // Listen for incoming TCP connections
    let mesh_listen = mesh.clone();
    tokio::spawn(async move {
        if let Err(e) = mesh_listen.listen().await {
            error!("Mesh listen error: {e}");
        }
    });

    // Run mDNS discovery
    tokio::spawn(async move {
        if let Err(e) = discovery.run().await {
            error!("Discovery error: {e}");
        }
    });

    // When a peer is discovered, connect to them via TCP
    let mesh_connect = mesh.clone();
    let my_peer_info = peer_info.clone();
    tokio::spawn(async move {
        while let Ok(event) = discovery_events.recv().await {
            match event {
                DiscoveryEvent::PeerFound { peer, addresses } => {
                    println!(
                        "  >>> Peer joined: {} ({:?}) at {:?}",
                        peer.name, peer.os, addresses
                    );

                    // Connect to peer's first IPv4 address
                    for addr in &addresses {
                        if addr.is_ipv4() {
                            let ip = addr.to_string();
                            if let Err(e) = mesh_connect.connect_to(&ip).await {
                                info!("Could not connect to {ip}: {e}");
                            } else {
                                // Send our info
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

    // Handle incoming mesh messages
    let clipboard_for_remote = clipboard.clone();
    let my_id = peer_id.0.to_string();
    tokio::spawn(async move {
        while let Ok(msg) = mesh_messages.recv().await {
            match msg {
                FlowMessage::Announce(peer) => {
                    info!("Received announce from: {} ({:?})", peer.name, peer.os);
                }
                FlowMessage::ClipboardData { peer_id, data, .. } => {
                    if peer_id.0.to_string() == my_id {
                        continue;
                    }
                    if let Ok(text) = String::from_utf8(data) {
                        println!("  [clipboard] Received from peer: {}...", &text[..text.len().min(60)]);
                        clipboard_for_remote.apply_remote(text).await;
                    }
                }
                _ => {}
            }
        }
    });

    // Monitor local clipboard and broadcast changes
    let mesh_clipboard = mesh.clone();
    let clipboard_id = PeerId(peer_id.0);
    tokio::spawn(async move {
        while let Ok(text) = clipboard_changes.recv().await {
            println!("  [clipboard] Sending to peers: {}...", &text[..text.len().min(60)]);
            let msg = FlowMessage::ClipboardData {
                peer_id: clipboard_id.clone(),
                content_type: ClipboardContentType::PlainText,
                data: text.into_bytes(),
            };
            if let Err(e) = mesh_clipboard.broadcast(&msg).await {
                error!("Failed to broadcast clipboard: {e}");
            }
        }
    });

    // Start watching clipboard
    let clipboard_watcher = clipboard.clone();
    tokio::spawn(async move {
        clipboard_watcher.watch().await;
    });

    println!("  Searching for peers on the network...");
    println!("  Clipboard sync is active.");
    println!("  Press Ctrl+C to stop.");
    println!();

    tokio::signal::ctrl_c().await?;
    println!("\n  FLOW agent stopped.");

    Ok(())
}
