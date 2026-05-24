use arboard::Clipboard;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

pub struct ClipboardSync {
    last_content: Arc<RwLock<String>>,
    last_remote_apply: Arc<RwLock<Instant>>,
    outgoing_tx: broadcast::Sender<String>,
}

impl ClipboardSync {
    pub fn new() -> Self {
        let (outgoing_tx, _) = broadcast::channel(16);
        Self {
            last_content: Arc::new(RwLock::new(String::new())),
            last_remote_apply: Arc::new(RwLock::new(Instant::now() - Duration::from_secs(10))),
            outgoing_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.outgoing_tx.subscribe()
    }

    pub async fn apply_remote(&self, content: String) {
        {
            let mut last = self.last_content.write().await;
            *last = content.clone();
        }
        {
            let mut ts = self.last_remote_apply.write().await;
            *ts = Instant::now();
        }

        std::thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to open clipboard for write: {e}");
                    return;
                }
            };
            if let Err(e) = clipboard.set_text(&content) {
                warn!("Failed to set clipboard: {e}");
            } else {
                info!("Clipboard updated from remote: {}...", &content[..content.len().min(50)]);
            }
        });
    }

    pub async fn watch(&self) {
        let last_content = self.last_content.clone();
        let last_remote = self.last_remote_apply.clone();
        let outgoing_tx = self.outgoing_tx.clone();

        tokio::task::spawn_blocking(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to open clipboard for monitoring: {e}");
                    return;
                }
            };

            loop {
                std::thread::sleep(Duration::from_millis(500));

                let current = match clipboard.get_text() {
                    Ok(text) => text,
                    Err(_) => continue,
                };

                let rt = tokio::runtime::Handle::current();

                // Skip if we recently applied a remote clipboard change (debounce)
                let since_remote = {
                    let ts = rt.block_on(last_remote.read());
                    ts.elapsed()
                };
                if since_remote < Duration::from_millis(1500) {
                    continue;
                }

                let last = rt.block_on(last_content.read()).clone();

                if !current.is_empty() && current != last {
                    debug!("Clipboard changed locally: {}...", &current[..current.len().min(50)]);
                    rt.block_on(async {
                        *last_content.write().await = current.clone();
                    });
                    let _ = outgoing_tx.send(current);
                }
            }
        })
        .await
        .ok();
    }
}
