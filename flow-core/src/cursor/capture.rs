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

#[derive(Debug, Clone)]
pub struct MouseDelta {
    pub dx: f64,
    pub dy: f64,
}

pub struct MouseCapture {
    pos_tx: broadcast::Sender<MousePosition>,
    delta_tx: broadcast::Sender<MouseDelta>,
    pub captured: Arc<AtomicBool>,
}

impl MouseCapture {
    pub fn new() -> Self {
        let (pos_tx, _) = broadcast::channel(128);
        let (delta_tx, _) = broadcast::channel(128);
        Self {
            pos_tx,
            delta_tx,
            captured: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn subscribe_pos(&self) -> broadcast::Receiver<MousePosition> {
        self.pos_tx.subscribe()
    }

    pub fn subscribe_delta(&self) -> broadcast::Receiver<MouseDelta> {
        self.delta_tx.subscribe()
    }

    pub fn enter_capture(&self) {
        if self.captured.swap(true, Ordering::SeqCst) { return; }
        info!("Mouse captured — entering remote mode");
        #[cfg(target_os = "macos")]
        {
            let d = core_graphics::display::CGDisplay::main();
            let _ = d.hide_cursor();
        }
    }

    pub fn exit_capture(&self) {
        if !self.captured.swap(false, Ordering::SeqCst) { return; }
        info!("Mouse released — back to local mode");
        #[cfg(target_os = "macos")]
        {
            let d = core_graphics::display::CGDisplay::main();
            let _ = d.show_cursor();
        }
    }

    pub fn is_captured(&self) -> bool {
        self.captured.load(Ordering::Relaxed)
    }

    #[cfg(target_os = "macos")]
    pub fn start_polling(&self) {
        use core_graphics::display::CGDisplay;
        use core_graphics::event::CGEvent;
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        let pos_tx = self.pos_tx.clone();
        let delta_tx = self.delta_tx.clone();
        let captured = self.captured.clone();

        std::thread::spawn(move || {
            info!("Mouse polling started (macOS)");

            let displays = CGDisplay::active_displays().unwrap_or_default();
            let mut total_w: f64 = 0.0;
            let mut total_h: f64 = 0.0;
            let mut min_x: f64 = 0.0;
            let mut min_y: f64 = 0.0;

            for &did in &displays {
                let d = CGDisplay::new(did);
                let b = d.bounds();
                let right = b.origin.x + b.size.width;
                let bottom = b.origin.y + b.size.height;
                if b.origin.x < min_x { min_x = b.origin.x; }
                if b.origin.y < min_y { min_y = b.origin.y; }
                if right > total_w { total_w = right; }
                if bottom > total_h { total_h = bottom; }
            }

            let sw = (total_w - min_x) as u32;
            let sh = (total_h - min_y) as u32;

            let mut last_x: f64 = 0.0;
            let mut last_y: f64 = 0.0;
            let mut warp_center_x: f64 = total_w / 2.0;
            let warp_center_y: f64 = total_h / 2.0;

            loop {
                std::thread::sleep(std::time::Duration::from_millis(8)); // ~120fps for smooth deltas

                let source = match CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let event = match CGEvent::new(source) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let loc = event.location();
                let x = loc.x;
                let y = loc.y;

                if captured.load(Ordering::Relaxed) {
                    // In capture mode: compute delta from center, warp back to center
                    let dx = x - warp_center_x;
                    let dy = y - warp_center_y;

                    if dx != 0.0 || dy != 0.0 {
                        let _ = delta_tx.send(MouseDelta { dx, dy });
                        // Warp cursor back to center
                        let _ = CGDisplay::warp_mouse_cursor_position(
                            core_graphics::geometry::CGPoint::new(warp_center_x, warp_center_y)
                        );
                    }
                } else {
                    // Normal mode: report position
                    let _ = pos_tx.send(MousePosition {
                        x: x - min_x,
                        y: y - min_y,
                        screen_width: sw,
                        screen_height: sh,
                        monitor_id: 0,
                    });
                }

                last_x = x;
                last_y = y;
            }
        });
    }

    #[cfg(target_os = "windows")]
    pub fn start_polling(&self) {
        let pos_tx = self.pos_tx.clone();
        let delta_tx = self.delta_tx.clone();
        let captured = self.captured.clone();

        std::thread::spawn(move || {
            info!("Mouse polling started (Windows)");
            loop {
                std::thread::sleep(std::time::Duration::from_millis(8));

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

                        if captured.load(Ordering::Relaxed) {
                            let cx = sw as i32 / 2;
                            let cy = sh as i32 / 2;
                            let dx = point.x - cx;
                            let dy = point.y - cy;
                            if dx != 0 || dy != 0 {
                                let _ = delta_tx.send(MouseDelta { dx: dx as f64, dy: dy as f64 });
                                use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
                                let _ = SetCursorPos(cx, cy);
                            }
                        } else {
                            let _ = pos_tx.send(MousePosition {
                                x: point.x as f64,
                                y: point.y as f64,
                                screen_width: sw,
                                screen_height: sh,
                                monitor_id: 0,
                            });
                        }
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
