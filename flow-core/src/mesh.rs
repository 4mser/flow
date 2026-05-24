use flow_protocol::FlowMessage;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

const MESH_PORT: u16 = 19847;

pub struct MeshNode {
    connections: Arc<RwLock<HashMap<String, TcpStream>>>,
    messages_tx: broadcast::Sender<FlowMessage>,
}

impl MeshNode {
    pub fn new() -> Self {
        let (messages_tx, _) = broadcast::channel(256);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            messages_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<FlowMessage> {
        self.messages_tx.subscribe()
    }

    pub async fn listen(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(("0.0.0.0", MESH_PORT)).await?;
        info!("Mesh listening on port {MESH_PORT}");

        let connections = self.connections.clone();
        let messages_tx = self.messages_tx.clone();

        loop {
            let (stream, addr) = listener.accept().await?;
            info!("Incoming connection from {addr}");
            let messages_tx = messages_tx.clone();
            let connections = connections.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, addr, messages_tx, connections).await {
                    warn!("Connection error from {addr}: {e}");
                }
            });
        }
    }

    pub async fn connect(&self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(addr).await?;
        info!("Connected to peer at {addr}");
        let addr_str = addr.to_string();

        let messages_tx = self.messages_tx.clone();
        let connections = self.connections.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, addr, messages_tx, connections).await {
                warn!("Connection error to {addr_str}: {e}");
            }
        });

        Ok(())
    }

    pub async fn broadcast(&self, msg: &FlowMessage) -> Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::to_vec(msg)?;
        let len = (data.len() as u32).to_be_bytes();

        let mut connections = self.connections.write().await;
        let mut dead = Vec::new();

        for (addr, stream) in connections.iter_mut() {
            if stream.write_all(&len).await.is_err() || stream.write_all(&data).await.is_err() {
                dead.push(addr.clone());
            }
        }

        for addr in dead {
            connections.remove(&addr);
        }

        Ok(())
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    messages_tx: broadcast::Sender<FlowMessage>,
    _connections: Arc<RwLock<HashMap<String, TcpStream>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut len_buf = [0u8; 4];

    loop {
        if stream.read_exact(&mut len_buf).await.is_err() {
            info!("Peer {addr} disconnected");
            break;
        }

        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 10 * 1024 * 1024 {
            warn!("Message too large from {addr}: {len} bytes");
            break;
        }

        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await?;

        match serde_json::from_slice::<FlowMessage>(&buf) {
            Ok(msg) => {
                let _ = messages_tx.send(msg);
            }
            Err(e) => {
                warn!("Invalid message from {addr}: {e}");
            }
        }
    }

    Ok(())
}
