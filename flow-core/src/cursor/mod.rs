pub mod capture;
pub mod input;
pub mod overlay;

use flow_protocol::{CursorColor, CursorPosition, Edge, PeerId, SpatialLayout};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

pub struct CursorManager {
    pub layout: Arc<RwLock<SpatialLayout>>,
    pub my_peer_id: PeerId,
    outgoing_tx: broadcast::Sender<CursorEvent>,
}

#[derive(Debug, Clone)]
pub enum CursorEvent {
    CrossedEdge {
        to_peer_id: PeerId,
        to_monitor: u32,
        position: CursorPosition,
    },
    RemoteCursorMoved {
        peer_id: PeerId,
        name: String,
        color: CursorColor,
        x: f64,
        y: f64,
        visible: bool,
    },
}

impl CursorManager {
    pub fn new(peer_id: PeerId) -> Self {
        let (outgoing_tx, _) = broadcast::channel(256);
        Self {
            layout: Arc::new(RwLock::new(SpatialLayout::new())),
            my_peer_id: peer_id,
            outgoing_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CursorEvent> {
        self.outgoing_tx.subscribe()
    }

    pub async fn check_edge(&self, x: f64, y: f64, screen_w: u32, screen_h: u32, monitor_id: u32) -> Option<CursorEvent> {
        let margin = 1.0;
        let edge;
        let ratio;

        if x <= margin {
            edge = Edge::Left;
            ratio = y / screen_h as f64;
        } else if x >= (screen_w as f64 - margin) {
            edge = Edge::Right;
            ratio = y / screen_h as f64;
        } else if y <= margin {
            edge = Edge::Top;
            ratio = x / screen_w as f64;
        } else if y >= (screen_h as f64 - margin) {
            edge = Edge::Bottom;
            ratio = x / screen_w as f64;
        } else {
            return None;
        }

        let layout = self.layout.read().await;
        if let Some((target, pos)) = layout.find_neighbor(&self.my_peer_id, monitor_id, edge, ratio) {
            if target.peer_id.0 != self.my_peer_id.0 {
                return Some(CursorEvent::CrossedEdge {
                    to_peer_id: target.peer_id.clone(),
                    to_monitor: target.monitor_id,
                    position: pos,
                });
            }
        }

        None
    }

    pub fn emit(&self, event: CursorEvent) {
        let _ = self.outgoing_tx.send(event);
    }
}
