use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::broadcast;
use tracing::info;

#[derive(Debug, Clone)]
pub struct MousePosition {
    pub x: f64,
    pub y: f64,
    pub screen_width: u32,
    pub screen_height: u32,
    pub monitor_id: u32,
}

pub struct MouseCapture {
    tx: broadcast::Sender<MousePosition>,
    pub captured: Arc<AtomicBool>,
}

impl MouseCapture {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(128);
        Self {
            tx,
            captured: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MousePosition> {
        self.tx.subscribe()
    }

    pub fn set_captured(&self, val: bool) {
        self.captured.store(val, Ordering::Relaxed);
    }

    pub fn is_captured(&self) -> bool {
        self.captured.load(Ordering::Relaxed)
    }

    #[cfg(target_os = "macos")]
    pub fn start_polling(&self) {
        use core_graphics::display::CGDisplay;
        use core_graphics::event::CGEvent;
        use core_graphics::event_source::CGEventSource;
        use core_graphics::event_source::CGEventSourceStateID;

        let tx = self.tx.clone();

        std::thread::spawn(move || {
            info!("Mouse capture polling started (macOS)");
            loop {
                std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps

                let event = match CGEvent::new(
                    CGEventSource::new(CGEventSourceStateID::CombinedSessionState).unwrap()
                ) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let loc = event.location();

                let displays = CGDisplay::active_displays().unwrap_or_default();
                let main = if displays.is_empty() { continue } else { CGDisplay::new(displays[0]) };
                let bounds = main.bounds();

                let _ = tx.send(MousePosition {
                    x: loc.x,
                    y: loc.y,
                    screen_width: bounds.size.width as u32,
                    screen_height: bounds.size.height as u32,
                    monitor_id: displays[0],
                });
            }
        });
    }

    #[cfg(target_os = "windows")]
    pub fn start_polling(&self) {
        let tx = self.tx.clone();

        std::thread::spawn(move || {
            info!("Mouse capture polling started (Windows)");
            loop {
                std::thread::sleep(std::time::Duration::from_millis(16));

                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
                    use windows::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, HORZRES, VERTRES};
                    use windows::Win32::Foundation::POINT;

                    let mut point = POINT::default();
                    if GetCursorPos(&mut point).is_ok() {
                        let hdc = GetDC(None);
                        let sw = GetDeviceCaps(hdc, HORZRES) as u32;
                        let sh = GetDeviceCaps(hdc, VERTRES) as u32;
                        let _ = ReleaseDC(None, hdc);

                        let _ = tx.send(MousePosition {
                            x: point.x as f64,
                            y: point.y as f64,
                            screen_width: sw,
                            screen_height: sh,
                            monitor_id: 0,
                        });
                    }
                }
            }
        });
    }

    #[cfg(target_os = "linux")]
    pub fn start_polling(&self) {
        info!("Mouse capture not yet implemented on Linux");
    }
}

#[cfg(target_os = "macos")]
pub fn warp_cursor(x: f64, y: f64) {
    use core_graphics::display::CGDisplay;
    use core_graphics::geometry::CGPoint;
    let _ = CGDisplay::warp_mouse_cursor_position(CGPoint::new(x, y));
}

#[cfg(target_os = "windows")]
pub fn warp_cursor(x: f64, y: f64) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
        let _ = SetCursorPos(x as i32, y as i32);
    }
}

#[cfg(target_os = "linux")]
pub fn warp_cursor(_x: f64, _y: f64) {}
