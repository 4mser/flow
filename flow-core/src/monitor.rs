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
            let scale = display
                .display_mode()
                .map(|m| m.pixel_width() as f64 / bounds.size.width)
                .unwrap_or(1.0);

            MonitorInfo {
                monitor_id: display_id,
                name: format!("Display {}", i + 1),
                width: bounds.size.width as u32,
                height: bounds.size.height as u32,
                scale_factor: scale,
                position: GridPosition {
                    x: bounds.origin.x as i32,
                    y: bounds.origin.y as i32,
                },
            }
        })
        .collect()
}

#[cfg(target_os = "windows")]
pub fn detect_monitors() -> Vec<MonitorInfo> {
    use std::mem;
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayDevicesW, EnumDisplaySettingsW, DEVMODEW, DISPLAY_DEVICEW,
        ENUM_CURRENT_SETTINGS,
    };

    let mut monitors = Vec::new();
    let mut index: u32 = 0;

    loop {
        let mut device: DISPLAY_DEVICEW = unsafe { mem::zeroed() };
        device.cb = mem::size_of::<DISPLAY_DEVICEW>() as u32;

        let ok = unsafe { EnumDisplayDevicesW(None, index, &mut device, 0) };
        if !ok.as_bool() {
            break;
        }

        if device.StateFlags & 1 == 0 {
            index += 1;
            continue;
        }

        let mut devmode: DEVMODEW = unsafe { mem::zeroed() };
        devmode.dmSize = mem::size_of::<DEVMODEW>() as u16;

        let ok = unsafe {
            EnumDisplaySettingsW(
                PCWSTR(device.DeviceName.as_ptr()),
                ENUM_CURRENT_SETTINGS,
                &mut devmode,
            )
        };

        if ok.as_bool() {
            let name_chars: Vec<u16> = device.DeviceName.iter().take_while(|&&c| c != 0).copied().collect();
            let name = String::from_utf16_lossy(&name_chars);

            monitors.push(MonitorInfo {
                monitor_id: index,
                name,
                width: devmode.dmPelsWidth,
                height: devmode.dmPelsHeight,
                scale_factor: 1.0,
                position: GridPosition {
                    x: unsafe { devmode.Anonymous1.Anonymous2.dmPosition.x },
                    y: unsafe { devmode.Anonymous1.Anonymous2.dmPosition.y },
                },
            });
        }

        index += 1;
    }

    if monitors.is_empty() {
        monitors.push(MonitorInfo {
            monitor_id: 0,
            name: "Primary Display".to_string(),
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            position: GridPosition::default(),
        });
    }

    monitors
}

#[cfg(target_os = "linux")]
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
