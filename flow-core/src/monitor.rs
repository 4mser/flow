use flow_protocol::{GridPosition, MonitorInfo};

#[cfg(target_os = "macos")]
pub fn detect_monitors() -> Vec<MonitorInfo> {
    use core_graphics::display::CGDisplay;

    let displays = CGDisplay::active_displays().unwrap_or_default();
    displays
        .into_iter()
        .enumerate()
        .map(|(i, display_id)| {
            let display = CGDisplay::new(display_id);
            let bounds = display.bounds();
            let mode = display.display_mode().unwrap();

            MonitorInfo {
                monitor_id: display_id,
                name: format!("Display {}", i + 1),
                width: bounds.size.width as u32,
                height: bounds.size.height as u32,
                scale_factor: mode.pixel_width() as f64 / bounds.size.width,
                position: GridPosition {
                    x: bounds.origin.x as i32,
                    y: bounds.origin.y as i32,
                },
            }
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn detect_monitors() -> Vec<MonitorInfo> {
    vec![MonitorInfo {
        monitor_id: 0,
        name: "Primary Display".to_string(),
        width: 1920,
        height: 1080,
        scale_factor: 1.0,
        position: GridPosition::default(),
    }]
}
