use clap::Parser;
use flow_core::discovery::{Discovery, DiscoveryEvent};
use flow_core::monitor::detect_monitors;
use flow_protocol::{CursorColor, OsType, PeerId, PeerInfo};
use tracing::{info, error};

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
    info!(
        "FLOW agent starting as '{}' with {} monitor(s)",
        peer_name,
        monitors.len()
    );
    for m in &monitors {
        info!(
            "  {} — {}x{} @ ({}, {}) scale={}",
            m.name, m.width, m.height, m.position.x, m.position.y, m.scale_factor
        );
    }

    let _peer_info = PeerInfo {
        id: peer_id.clone(),
        name: peer_name.clone(),
        color,
        monitors,
        os: OsType::current(),
    };

    let discovery = Discovery::new(&peer_name, &peer_id, color)?;
    let mut events = discovery.subscribe();

    info!("Searching for peers on the network...");

    let discovery_handle = tokio::spawn(async move {
        if let Err(e) = discovery.run().await {
            error!("Discovery error: {e}");
        }
    });

    let events_handle = tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                DiscoveryEvent::PeerFound(peer) => {
                    info!(
                        ">>> Peer joined: {} ({}:{:?}) — color: rgb({},{},{})",
                        peer.name,
                        peer.id.0,
                        peer.os,
                        peer.color.r,
                        peer.color.g,
                        peer.color.b,
                    );
                }
                DiscoveryEvent::PeerLost(id) => {
                    info!("<<< Peer left: {id}");
                }
            }
        }
    });

    tokio::signal::ctrl_c().await?;
    info!("Shutting down FLOW agent...");

    Ok(())
}
