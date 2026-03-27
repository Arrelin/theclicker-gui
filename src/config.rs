#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct HotkeyBind {
    pub key: u16,
    pub mods: u8,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
#[serde(default)]
pub struct Config {
    pub device_name: String,
    pub cooldown: u64,
    pub cooldown_press_release: u64,
    pub enable_lock_unlock: bool,
    pub lock_unlock_bind: Option<u16>,
    pub enable_left: bool,
    pub left_bind: Option<u16>,
    pub enable_middle: bool,
    pub middle_bind: Option<u16>,
    pub enable_right: bool,
    pub right_bind: Option<u16>,
    pub hold: bool,
    pub grab: bool,
    pub no_min_delay: bool,
    pub enable_hotkey: bool,
    pub hotkey_bind: Option<HotkeyBind>,
}

impl Config {
    pub fn missing_binds(&self) -> bool {
        (self.enable_lock_unlock && self.lock_unlock_bind.is_none())
            || (self.enable_left && self.left_bind.is_none())
            || (self.enable_middle && self.middle_bind.is_none())
            || (self.enable_right && self.right_bind.is_none())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_name: String::new(),
            cooldown: 1,
            cooldown_press_release: 0,
            enable_lock_unlock: false,
            lock_unlock_bind: None,
            enable_left: true,
            left_bind: None,
            enable_middle: false,
            middle_bind: None,
            enable_right: false,
            right_bind: None,
            hold: true,
            grab: true,
            no_min_delay: false,
            enable_hotkey: false,
            hotkey_bind: None,
        }
    }
}
