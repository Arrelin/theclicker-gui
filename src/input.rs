use theclicker::InputDevice;

use crate::config::HotkeyBind;

pub fn clean_name(s: &str) -> &str {
    s.trim_matches('\0').trim()
}

pub fn load_devices() -> Vec<(String, String)> {
    let mut devices = InputDevice::devices();
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    devices
        .into_iter()
        .map(|d| {
            let display = clean_name(&d.name).to_string();
            let base_name = display
                .strip_suffix(&format!("-{}", d.filename))
                .unwrap_or(&display)
                .to_string();
            (display, base_name)
        })
        .collect()
}

pub fn key_label(code: u16) -> String {
    if let Ok(key) = input_linux::Key::from_code(code) {
        format!("{key:?} ({code})")
    } else {
        format!("KeyCode {code}")
    }
}

pub fn modifier_bit(code: u16) -> u8 {
    match code {
        29 | 97 => 1,
        56 | 100 => 2,
        42 | 54 => 4,
        125 | 126 => 8,
        _ => 0,
    }
}

pub fn hotkey_label(bind: &HotkeyBind) -> String {
    let mut s = String::new();
    if bind.mods & 1 != 0 { s.push_str("Ctrl+"); }
    if bind.mods & 8 != 0 { s.push_str("Super+"); }
    if bind.mods & 2 != 0 { s.push_str("Alt+"); }
    if bind.mods & 4 != 0 { s.push_str("Shift+"); }
    s.push_str(&key_label(bind.key));
    s
}
