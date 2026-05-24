use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerId(pub Uuid);

impl PeerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: PeerId,
    pub name: String,
    pub color: CursorColor,
    pub monitors: Vec<MonitorInfo>,
    pub os: OsType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OsType {
    MacOS,
    Windows,
    Linux,
}

impl OsType {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOS
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub monitor_id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub position: GridPosition,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct GridPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CursorColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl CursorColor {
    pub const RED: Self = Self { r: 239, g: 68, b: 68 };
    pub const BLUE: Self = Self { r: 59, g: 130, b: 246 };
    pub const GREEN: Self = Self { r: 34, g: 197, b: 94 };
    pub const PURPLE: Self = Self { r: 168, g: 85, b: 247 };
    pub const ORANGE: Self = Self { r: 249, g: 115, b: 22 };
    pub const PINK: Self = Self { r: 236, g: 72, b: 153 };

    pub fn palette() -> &'static [Self] {
        &[Self::RED, Self::BLUE, Self::GREEN, Self::PURPLE, Self::ORANGE, Self::PINK]
    }

    pub fn pick(index: usize) -> Self {
        let palette = Self::palette();
        palette[index % palette.len()]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PeerStatus {
    Active,
    Idle,
    Away,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlowMessage {
    Announce(PeerInfo),
    CursorMove {
        peer_id: PeerId,
        monitor_id: u32,
        position: CursorPosition,
    },
    CursorHandoff {
        from_peer: PeerId,
        to_peer: PeerId,
        to_monitor: u32,
        position: CursorPosition,
    },
    ClipboardOffer {
        peer_id: PeerId,
        content_type: ClipboardContentType,
        size_bytes: u64,
    },
    ClipboardRequest {
        from_peer: PeerId,
        to_peer: PeerId,
    },
    ClipboardData {
        peer_id: PeerId,
        content_type: ClipboardContentType,
        data: Vec<u8>,
    },
    StatusUpdate {
        peer_id: PeerId,
        status: PeerStatus,
    },
    Goodbye(PeerId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContentType {
    PlainText,
    RichText,
    Image,
    Files(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialLayout {
    pub entries: Vec<SpatialEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialEntry {
    pub peer_id: PeerId,
    pub monitor_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl SpatialLayout {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_peer_monitors(&mut self, peer: &PeerInfo) {
        for monitor in &peer.monitors {
            self.entries.push(SpatialEntry {
                peer_id: peer.id.clone(),
                monitor_id: monitor.monitor_id,
                x: monitor.position.x,
                y: monitor.position.y,
                width: monitor.width,
                height: monitor.height,
            });
        }
    }

    pub fn find_neighbor(
        &self,
        from_peer: &PeerId,
        from_monitor: u32,
        edge: Edge,
        cursor_ratio: f64,
    ) -> Option<(&SpatialEntry, CursorPosition)> {
        let source = self.entries.iter().find(|e| {
            e.peer_id.0 == from_peer.0 && e.monitor_id == from_monitor
        })?;

        let (check_x, check_y) = match edge {
            Edge::Right => (
                source.x + source.width as i32 + 1,
                source.y + (source.height as f64 * cursor_ratio) as i32,
            ),
            Edge::Left => (
                source.x - 1,
                source.y + (source.height as f64 * cursor_ratio) as i32,
            ),
            Edge::Top => (
                source.x + (source.width as f64 * cursor_ratio) as i32,
                source.y - 1,
            ),
            Edge::Bottom => (
                source.x + (source.width as f64 * cursor_ratio) as i32,
                source.y + source.height as i32 + 1,
            ),
        };

        self.entries
            .iter()
            .filter(|e| !(e.peer_id.0 == from_peer.0 && e.monitor_id == from_monitor))
            .find(|e| {
                check_x >= e.x
                    && check_x < e.x + e.width as i32
                    && check_y >= e.y
                    && check_y < e.y + e.height as i32
            })
            .map(|target| {
                let pos = CursorPosition {
                    x: (check_x - target.x) as f64,
                    y: (check_y - target.y) as f64,
                };
                (target, pos)
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}
