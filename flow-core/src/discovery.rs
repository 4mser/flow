use flow_protocol::{CursorColor, OsType, PeerId, PeerInfo};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

const SERVICE_TYPE: &str = "_flow._tcp.local.";
const SERVICE_PORT: u16 = 19847;

pub struct Discovery {
    daemon: ServiceDaemon,
    peer_id: PeerId,
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
    events_tx: broadcast::Sender<DiscoveryEvent>,
}

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerFound(PeerInfo),
    PeerLost(String),
}

impl Discovery {
    pub fn new(peer_name: &str, peer_id: &PeerId, color: CursorColor) -> Result<Self, Box<dyn std::error::Error>> {
        let daemon = ServiceDaemon::new()?;
        let (events_tx, _) = broadcast::channel(64);

        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let instance_name = format!("flow-{}", &peer_id.0.to_string()[..8]);
        let properties = [
            ("peer_id", peer_id.0.to_string()),
            ("name", peer_name.to_owned()),
            ("os", serde_json::to_string(&OsType::current()).unwrap()),
            ("color_r", color.r.to_string()),
            ("color_g", color.g.to_string()),
            ("color_b", color.b.to_string()),
        ];
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &format!("{hostname}.local."),
            "",
            SERVICE_PORT,
            &properties[..],
        )?;

        daemon.register(service)?;
        info!("Registered mDNS service: {instance_name}");

        Ok(Self {
            daemon,
            peer_id: peer_id.clone(),
            peers: Arc::new(RwLock::new(HashMap::new())),
            events_tx,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DiscoveryEvent> {
        self.events_tx.subscribe()
    }

    pub async fn peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.values().cloned().collect()
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let receiver = self.daemon.browse(SERVICE_TYPE)?;
        let peers = self.peers.clone();
        let events_tx = self.events_tx.clone();
        let my_id = self.peer_id.0.to_string();

        loop {
            match receiver.recv_async().await {
                Ok(event) => match event {
                    ServiceEvent::ServiceResolved(service) => {
                        let props = service.get_properties();
                        let peer_id_str = props
                            .get_property_val_str("peer_id")
                            .unwrap_or_default();

                        if peer_id_str == my_id {
                            continue;
                        }

                        let name = props
                            .get_property_val_str("name")
                            .unwrap_or("unknown")
                            .to_string();

                        let color = CursorColor {
                            r: props.get_property_val_str("color_r")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(255),
                            g: props.get_property_val_str("color_g")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(255),
                            b: props.get_property_val_str("color_b")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(255),
                        };

                        let peer_info = PeerInfo {
                            id: PeerId(peer_id_str.parse().unwrap_or_else(|_| uuid::Uuid::new_v4())),
                            name: name.clone(),
                            color,
                            monitors: vec![],
                            os: OsType::current(),
                        };

                        info!("Peer found: {name} ({peer_id_str})");
                        peers.write().await.insert(peer_id_str.to_string(), peer_info.clone());
                        let _ = events_tx.send(DiscoveryEvent::PeerFound(peer_info));
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        info!("Peer left: {fullname}");
                        let mut peers = peers.write().await;
                        let removed: Vec<String> = peers
                            .iter()
                            .filter(|(_, p)| fullname.contains(&p.id.0.to_string()[..8]))
                            .map(|(k, _)| k.clone())
                            .collect();
                        for key in removed {
                            peers.remove(&key);
                            let _ = events_tx.send(DiscoveryEvent::PeerLost(key));
                        }
                    }
                    _ => {}
                },
                Err(e) => {
                    warn!("mDNS browse error: {e}");
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn shutdown(self) -> Result<(), Box<dyn std::error::Error>> {
        self.daemon.shutdown()?;
        Ok(())
    }
}
