use flow_protocol::FlowMessage;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

pub const MESH_PORT: u16 = 19847;
const MAX_MSG_SIZE: usize = 64 * 1024 * 1024; // 64MB

pub struct MeshNode {
    writers: Arc<RwLock<Vec<WriterEntry>>>,
    connected_ips: Arc<RwLock<HashSet<String>>>,
    incoming_tx: broadcast::Sender<FlowMessage>,
}

struct WriterEntry {
    writer: Arc<tokio::sync::Mutex<tokio::io::WriteHalf<TcpStream>>>,
    addr: String,
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
            let ip = addr.ip().to_string();

            {
                let connected = self.connected_ips.read().await;
                if connected.contains(&ip) {
                    info!("Duplicate connection from {ip}, dropping");
                    continue;
                }
            }

            info!("Incoming connection from {addr}");
            self.connected_ips.write().await.insert(ip);
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

        self.connected_ips.write().await.insert(ip.to_string());

        let addr: SocketAddr = format!("{ip}:{MESH_PORT}").parse()?;
        match TcpStream::connect(addr).await {
            Ok(stream) => {
                info!("Connected to peer at {addr}");
                self.handle_stream(stream, addr).await;
                Ok(())
            }
            Err(e) => {
                self.connected_ips.write().await.remove(ip);
                Err(e.into())
            }
        }
    }

    async fn handle_stream(&self, stream: TcpStream, addr: SocketAddr) {
        let (reader, writer) = tokio::io::split(stream);
        let writer_arc = Arc::new(tokio::sync::Mutex::new(writer));
        let addr_str = addr.ip().to_string();

        self.writers.write().await.push(WriterEntry {
            writer: writer_arc.clone(),
            addr: addr_str.clone(),
        });

        let incoming_tx = self.incoming_tx.clone();
        let writers = self.writers.clone();
        let connected_ips = self.connected_ips.clone();

        tokio::spawn(async move {
            let mut reader = reader;
            let mut len_buf = [0u8; 4];

            loop {
                if reader.read_exact(&mut len_buf).await.is_err() {
                    info!("Peer {addr} disconnected");
                    break;
                }

                let len = u32::from_be_bytes(len_buf) as usize;
                if len > MAX_MSG_SIZE {
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

            // Cleanup on disconnect
            writers.write().await.retain(|e| !Arc::ptr_eq(&e.writer, &writer_arc));
            connected_ips.write().await.remove(&addr_str);
            info!("Cleaned up connection to {addr}");
        });
    }

    pub async fn broadcast(&self, msg: &FlowMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = serde_json::to_vec(msg)?;
        let len = (data.len() as u32).to_be_bytes();

        let writers = self.writers.read().await;
        let mut failed = Vec::new();

        for (i, entry) in writers.iter().enumerate() {
            let mut w = entry.writer.lock().await;
            if w.write_all(&len).await.is_err() || w.write_all(&data).await.is_err() {
                warn!("Failed to send to peer {}", entry.addr);
                failed.push(i);
            }
        }
        drop(writers);

        if !failed.is_empty() {
            let mut writers = self.writers.write().await;
            for &i in failed.iter().rev() {
                if i < writers.len() {
                    let removed = writers.remove(i);
                    self.connected_ips.write().await.remove(&removed.addr);
                }
            }
        }

        Ok(())
    }

    pub async fn send_raw(&self, data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let len = (data.len() as u32).to_be_bytes();
        let writers = self.writers.read().await;

        for entry in writers.iter() {
            let mut w = entry.writer.lock().await;
            let _ = w.write_all(&len).await;
            let _ = w.write_all(data).await;
        }

        Ok(())
    }

    pub async fn peer_count(&self) -> usize {
        self.writers.read().await.len()
    }
}
