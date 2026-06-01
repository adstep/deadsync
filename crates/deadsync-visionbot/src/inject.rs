//! Windows key injection via `SendInput` (scancode), plus a foreground guard.
//!
//! The game reads input through a raw-input backend, so we inject **scancodes**
//! (`KEYEVENTF_SCANCODE`) rather than virtual keys; arrow keys additionally need
//! `KEYEVENTF_EXTENDEDKEY`. We never inject unless DeadSync is the foreground
//! window, so stray presses cannot leak into other apps.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// A keyboard scancode plus whether it is an "extended" key (E0-prefixed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanKey {
    pub scancode: u16,
    pub extended: bool,
}

/// Resolve a calibration key name to a scancode. Supports the four arrow keys
/// (extended) and the `WASD`/`ASDF`-style fallbacks (non-extended).
pub fn resolve_key(name: &str) -> Option<ScanKey> {
    let k = name.trim().to_ascii_lowercase();
    let key = match k.as_str() {
        "left" | "arrowleft" => ScanKey {
            scancode: 0x4B,
            extended: true,
        },
        "right" | "arrowright" => ScanKey {
            scancode: 0x4D,
            extended: true,
        },
        "up" | "arrowup" => ScanKey {
            scancode: 0x48,
            extended: true,
        },
        "down" | "arrowdown" => ScanKey {
            scancode: 0x50,
            extended: true,
        },
        "a" => ScanKey {
            scancode: 0x1E,
            extended: false,
        },
        "s" => ScanKey {
            scancode: 0x1F,
            extended: false,
        },
        "w" => ScanKey {
            scancode: 0x11,
            extended: false,
        },
        "d" => ScanKey {
            scancode: 0x20,
            extended: false,
        },
        "f" => ScanKey {
            scancode: 0x21,
            extended: false,
        },
        "j" => ScanKey {
            scancode: 0x24,
            extended: false,
        },
        "k" => ScanKey {
            scancode: 0x25,
            extended: false,
        },
        "l" => ScanKey {
            scancode: 0x26,
            extended: false,
        },
        _ => return None,
    };
    Some(key)
}

fn make_input(key: ScanKey, keyup: bool) -> INPUT {
    let mut raw = KEYEVENTF_SCANCODE.0;
    if key.extended {
        raw |= KEYEVENTF_EXTENDEDKEY.0;
    }
    if keyup {
        raw |= KEYEVENTF_KEYUP.0;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: key.scancode,
                dwFlags: KEYBD_EVENT_FLAGS(raw),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Is `hwnd` the current foreground window?
pub fn is_foreground(hwnd: HWND) -> bool {
    unsafe { GetForegroundWindow() == hwnd }
}

/// Send a keydown for `key`. Returns the number of events inserted.
pub fn key_down(key: ScanKey) -> u32 {
    let inputs = [make_input(key, false)];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) }
}

/// Send a keyup for `key`.
pub fn key_up(key: ScanKey) -> u32 {
    let inputs = [make_input(key, true)];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_arrow_and_fallback_keys() {
        assert_eq!(
            resolve_key("Left"),
            Some(ScanKey {
                scancode: 0x4B,
                extended: true
            })
        );
        assert_eq!(
            resolve_key("d"),
            Some(ScanKey {
                scancode: 0x20,
                extended: false
            })
        );
        assert!(resolve_key("nonsense").is_none());
    }

    #[test]
    fn keyup_flag_distinguishes_events() {
        let down = make_input(
            ScanKey {
                scancode: 0x4B,
                extended: true,
            },
            false,
        );
        let up = make_input(
            ScanKey {
                scancode: 0x4B,
                extended: true,
            },
            true,
        );
        unsafe {
            assert_eq!(down.Anonymous.ki.dwFlags.0 & KEYEVENTF_KEYUP.0, 0);
            assert_ne!(up.Anonymous.ki.dwFlags.0 & KEYEVENTF_KEYUP.0, 0);
            // extended + scancode flags always present
            assert_ne!(down.Anonymous.ki.dwFlags.0 & KEYEVENTF_SCANCODE.0, 0);
            assert_ne!(down.Anonymous.ki.dwFlags.0 & KEYEVENTF_EXTENDEDKEY.0, 0);
        }
    }
}
