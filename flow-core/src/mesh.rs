use flow_protocol::FlowMessage;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

pub const MESH_PORT: u16 = 19847;

pub struct MeshNode {
    writers: Arc<RwLock<Vec<Arc<tokio::sync::Mutex<tokio::io::WriteHalf<TcpStream>>>>>>,
    connected_ips: Arc<RwLock<HashSet<String>>>,
    incoming_tx: broadcast::Sender<FlowMessage>,
}

impl MeshNode {
    pub fn new() -> Self {
        let (incoming_tx, _) = broadcast::channel(256);
        Self {
            writers: Arc::new(RwLock::new(Vec::new())),
            connected_ips: Arc::new(RwLock::new(HashSet::new())),
            incoming_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<FlowMessage> {
        self.incoming_tx.subscribe()
    }

    pub async fn listen(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(("0.0.0.0", MESH_PORT)).await?;
        info!("Mesh listening on port {MESH_PORT}");

        loop {
            let (stream, addr) = listener.accept().await?;
            info!("Incoming connection from {addr}");
            self.handle_stream(stream, addr).await;
        }
    }

    pub async fn connect_to(&self, ip: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        {
            let connected = self.connected_ips.read().await;
            if connected.contains(ip) {
                return Ok(());
            }
        }

        let addr: SocketAddr = format!("{ip}:{MESH_PORT}").parse()?;
        let stream = TcpStream::connect(addr).await?;
        info!("Connected to peer at {addr}");

        self.connected_ips.write().await.insert(ip.to_string());
        self.handle_stream(stream, addr).await;
        Ok(())
    }

    async fn handle_stream(&self, stream: TcpStream, addr: SocketAddr) {
        let (reader, writer) = tokio::io::split(stream);
        let writer = Arc::new(tokio::sync::Mutex::new(writer));
        self.writers.write().await.push(writer.clone());

        let incoming_tx = self.incoming_tx.clone();
        let writers = self.writers.clone();

        tokio::spawn(async move {
            let mut reader = reader;
            let mut len_buf = [0u8; 4];

            loop {
                if reader.read_exact(&mut len_buf).await.is_err() {
                    info!("Peer {addr} disconnected");
                    break;
                }

                let len = u32::from_be_bytes(len_buf) as usize;
                if len > 10 * 1024 * 1024 {
                    warn!("Message too large from {addr}: {len} bytes");
                    break;
                }

                let mut buf = vec![0u8; len];
                if reader.read_exact(&mut buf).await.is_err() {
                    break;
                }

                match serde_json::from_slice::<FlowMessage>(&buf) {
                    Ok(msg) => {
                        let _ = incoming_tx.send(msg);
                    }
                    Err(e) => {
                        warn!("Invalid message from {addr}: {e}");
                    }
                }
            }

            let mut w = writers.write().await;
            w.retain(|w| !Arc::ptr_eq(w, &writer));
        });
    }

    pub async fn broadcast(&self, msg: &FlowMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = serde_json::to_vec(msg)?;
        let len = (data.len() as u32).to_be_bytes();

        let writers = self.writers.read().await;
        for writer in writers.iter() {
            let mut w = writer.lock().await;
            if w.write_all(&len).await.is_err() || w.write_all(&data).await.is_err() {
                warn!("Failed to send to a peer");
            }
        }

        Ok(())
    }

    pub async fn has_connections(&self) -> bool {
        !self.writers.read().await.is_empty()
    }
}
