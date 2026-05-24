use flow_protocol::CursorColor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::info;

pub struct CursorOverlay {
    pub visible: Arc<AtomicBool>,
    pub x: Arc<std::sync::Mutex<f64>>,
    pub y: Arc<std::sync::Mutex<f64>>,
    pub color: Arc<std::sync::Mutex<CursorColor>>,
    pub name: Arc<std::sync::Mutex<String>>,
}

impl CursorOverlay {
    pub fn new() -> Self {
        Self {
            visible: Arc::new(AtomicBool::new(false)),
            x: Arc::new(std::sync::Mutex::new(0.0)),
            y: Arc::new(std::sync::Mutex::new(0.0)),
            color: Arc::new(std::sync::Mutex::new(CursorColor::BLUE)),
            name: Arc::new(std::sync::Mutex::new("Remote".to_string())),
        }
    }

    pub fn update(&self, x: f64, y: f64, color: CursorColor, name: &str, visible: bool) {
        *self.x.lock().unwrap() = x;
        *self.y.lock().unwrap() = y;
        *self.color.lock().unwrap() = color;
        *self.name.lock().unwrap() = name.to_string();
        self.visible.store(visible, Ordering::Relaxed);
    }

    pub fn hide(&self) {
        self.visible.store(false, Ordering::Relaxed);
    }

    #[cfg(target_os = "macos")]
    pub fn start_render_loop(&self) {
        use cocoa::appkit::{
            NSBackingStoreBuffered, NSWindow, NSWindowStyleMask, NSScreen,
        };
        use cocoa::base::{nil, YES, NO};
        use cocoa::foundation::{NSPoint, NSRect, NSSize, NSAutoreleasePool};
        use objc::rc::autoreleasepool;

        let visible = self.visible.clone();
        let x = self.x.clone();
        let y = self.y.clone();

        std::thread::spawn(move || {
            info!("Cursor overlay render loop started (macOS)");

            unsafe {
                let _pool = NSAutoreleasePool::new(nil);

                let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(16.0, 16.0)),
                    NSWindowStyleMask::NSBorderlessWindowMask,
                    NSBackingStoreBuffered,
                    NO,
                );

                // Level 25 = above everything, including the menu bar
                window.setLevel_(25);
                window.setOpaque_(NO);
                window.setHasShadow_(NO);
                window.setIgnoresMouseEvents_(YES);
                let bg_color = cocoa::appkit::NSColor::colorWithRed_green_blue_alpha_(
                    nil, 0.23, 0.27, 0.95, 0.9
                );
                window.setBackgroundColor_(bg_color);

                loop {
                    autoreleasepool(|| {
                        std::thread::sleep(std::time::Duration::from_millis(16));

                        if visible.load(Ordering::Relaxed) {
                            let cx = *x.lock().unwrap();
                            let cy = *y.lock().unwrap();

                            let screen: cocoa::base::id = NSScreen::mainScreen(nil);
                            let frame = NSScreen::frame(screen);
                            let flipped_y = frame.size.height - cy - 16.0;

                            window.setFrameOrigin_(NSPoint::new(cx, flipped_y));
                            window.orderFront_(nil);
                        } else {
                            window.orderOut_(nil);
                        }
                    });
                }
            }
        });
    }

    #[cfg(target_os = "windows")]
    pub fn start_render_loop(&self) {
        info!("Cursor overlay render loop started (Windows - placeholder)");
    }

    #[cfg(target_os = "linux")]
    pub fn start_render_loop(&self) {
        info!("Cursor overlay not yet implemented on Linux");
    }
}
