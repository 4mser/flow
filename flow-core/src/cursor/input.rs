/// Cross-platform input injection: move cursor, click, and type on remote machines.

// ─────────────────────────────── macOS ───────────────────────────────
#[cfg(target_os = "macos")]
mod platform {
    use core_graphics::event::{CGEvent, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    unsafe extern "C" {
        fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
        fn CGEventCreateKeyboardEvent(
            source: *const std::ffi::c_void,
            keycode: u16,
            key_down: bool,
        ) -> *mut std::ffi::c_void;
        fn CGEventSetFlags(event: *mut std::ffi::c_void, flags: u64);
        fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
        fn CFRelease(cf: *mut std::ffi::c_void);
    }

    pub fn inject_mouse_move(x: f64, y: f64) {
        unsafe {
            CGWarpMouseCursorPosition(CGPoint::new(x, y));
        }
    }

    pub fn inject_mouse_click(button: u8, pressed: bool, x: f64, y: f64) {
        let source = match CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
            Ok(s) => s,
            Err(_) => return,
        };

        let (event_type, cg_button) = match (button, pressed) {
            (0, true) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
            (0, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
            (1, true) => (CGEventType::RightMouseDown, CGMouseButton::Right),
            (1, false) => (CGEventType::RightMouseUp, CGMouseButton::Right),
            (2, true) => (CGEventType::OtherMouseDown, CGMouseButton::Center),
            (2, false) => (CGEventType::OtherMouseUp, CGMouseButton::Center),
            _ => return,
        };

        let point = CGPoint::new(x, y);
        if let Ok(event) = CGEvent::new_mouse_event(source, event_type, point, cg_button) {
            event.post(core_graphics::event::CGEventTapLocation::HID);
        }
    }

    pub fn inject_key(key_code: u16, pressed: bool, flags: u64) {
        unsafe {
            let event = CGEventCreateKeyboardEvent(std::ptr::null(), key_code, pressed);
            if !event.is_null() {
                CGEventSetFlags(event, flags);
                CGEventPost(0, event); // 0 = kCGHIDEventTap
                CFRelease(event);
            }
        }
    }
}

// ─────────────────────────────── Windows ─────────────────────────────
#[cfg(target_os = "windows")]
mod platform {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_TYPE,
        MOUSEINPUT, MOUSE_EVENT_FLAGS,
        KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    };
    use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

    pub fn inject_mouse_move(x: f64, y: f64) {
        unsafe {
            let _ = SetCursorPos(x as i32, y as i32);
        }
    }

    pub fn inject_mouse_click(button: u8, pressed: bool, x: f64, y: f64) {
        inject_mouse_move(x, y);

        let flags: u32 = match (button, pressed) {
            (0, true) => 0x0002,  // MOUSEEVENTF_LEFTDOWN
            (0, false) => 0x0004, // MOUSEEVENTF_LEFTUP
            (1, true) => 0x0008,  // MOUSEEVENTF_RIGHTDOWN
            (1, false) => 0x0010, // MOUSEEVENTF_RIGHTUP
            (2, true) => 0x0020,  // MOUSEEVENTF_MIDDLEDOWN
            (2, false) => 0x0040, // MOUSEEVENTF_MIDDLEUP
            _ => return,
        };

        let input = INPUT {
            r#type: INPUT_TYPE(0), // INPUT_MOUSE
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSE_EVENT_FLAGS(flags),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
    }

    pub fn inject_key(key_code: u16, pressed: bool, _flags: u64) {
        let mut flags = KEYEVENTF_SCANCODE;
        if !pressed {
            flags |= KEYEVENTF_KEYUP;
        }

        let input = INPUT {
            r#type: INPUT_TYPE(1), // INPUT_KEYBOARD
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
                    wScan: key_code,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
    }
}

// ─────────────────────────────── Linux ───────────────────────────────
#[cfg(target_os = "linux")]
mod platform {
    pub fn inject_mouse_move(_x: f64, _y: f64) {
        tracing::warn!("inject_mouse_move not yet implemented on Linux");
    }

    pub fn inject_mouse_click(_button: u8, _pressed: bool, _x: f64, _y: f64) {
        tracing::warn!("inject_mouse_click not yet implemented on Linux");
    }

    pub fn inject_key(_key_code: u16, _pressed: bool, _flags: u64) {
        tracing::warn!("inject_key not yet implemented on Linux");
    }
}

// Re-export platform functions at module level
pub use platform::{inject_key, inject_mouse_click, inject_mouse_move};
