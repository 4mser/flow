use flow_protocol::{FlowMessage, PeerId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::mesh::MeshNode;

const CHUNK_SIZE: usize = 64 * 1024; // 64KB

pub struct FileTransferManager {
    active_receives: Arc<RwLock<HashMap<Uuid, ReceiveState>>>,
    download_dir: PathBuf,
}

struct ReceiveState {
    file_name: String,
    file_size: u64,
    bytes_received: u64,
    file: tokio::fs::File,
}

impl FileTransferManager {
    pub fn new() -> Self {
        let download_dir = dirs_next().join("FLOW");
        Self {
            active_receives: Arc::new(RwLock::new(HashMap::new())),
            download_dir,
        }
    }

    pub async fn send_file(
        &self,
        path: &Path,
        my_peer_id: &PeerId,
        mesh: &MeshNode,
    ) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        let metadata = fs::metadata(path).await?;
        let file_size = metadata.len();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let transfer_id = Uuid::new_v4();

        info!("Sending file: {file_name} ({file_size} bytes), transfer_id: {transfer_id}");

        let offer = FlowMessage::FileOffer {
            transfer_id,
            from_peer: my_peer_id.clone(),
            file_name: file_name.clone(),
            file_size,
        };
        mesh.broadcast(&offer).await?;

        let mut file = tokio::fs::File::open(path).await?;
        let mut offset: u64 = 0;
        let mut buf = vec![0u8; CHUNK_SIZE];

        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }

            let is_last = offset + n as u64 >= file_size;
            let chunk = FlowMessage::FileChunk {
                transfer_id,
                offset,
                data: buf[..n].to_vec(),
                is_last,
            };
            mesh.broadcast(&chunk).await?;

            offset += n as u64;

            if offset % (1024 * 1024) < CHUNK_SIZE as u64 {
                info!("File transfer progress: {:.1}%", (offset as f64 / file_size as f64) * 100.0);
            }

            // Small yield to not flood the connection
            tokio::task::yield_now().await;
        }

        let complete = FlowMessage::FileComplete {
            transfer_id,
            file_name: file_name.clone(),
        };
        mesh.broadcast(&complete).await?;

        info!("File sent: {file_name} ({offset} bytes)");
        Ok(transfer_id)
    }

    pub async fn handle_offer(
        &self,
        transfer_id: Uuid,
        file_name: &str,
        file_size: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        fs::create_dir_all(&self.download_dir).await?;

        let dest = self.download_dir.join(file_name);
        let file = tokio::fs::File::create(&dest).await?;

        info!("Receiving file: {file_name} ({file_size} bytes) -> {}", dest.display());

        self.active_receives.write().await.insert(
            transfer_id,
            ReceiveState {
                file_name: file_name.to_string(),
                file_size,
                bytes_received: 0,
                file,
            },
        );

        Ok(())
    }

    pub async fn handle_chunk(
        &self,
        transfer_id: Uuid,
        _offset: u64,
        data: &[u8],
        is_last: bool,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut receives = self.active_receives.write().await;
        let state = match receives.get_mut(&transfer_id) {
            Some(s) => s,
            None => return Ok(None),
        };

        state.file.write_all(data).await?;
        state.bytes_received += data.len() as u64;

        if is_last {
            state.file.flush().await?;
            let name = state.file_name.clone();
            let total = state.bytes_received;
            receives.remove(&transfer_id);
            info!("File received: {name} ({total} bytes) -> {}", self.download_dir.display());
            return Ok(Some(name));
        }

        Ok(None)
    }

    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }
}

fn dirs_next() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Public"))
            .join("Downloads")
    } else {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
            .join("Downloads")
    }
}
