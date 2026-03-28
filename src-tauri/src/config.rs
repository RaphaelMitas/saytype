use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Modifier key enum for hotkey configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Command,
    Shift,
    Option,
    Control,
    Function,
}

/// Hotkey configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// Modifier keys required (e.g., [Command], or [Control, Shift])
    pub modifiers: Vec<Modifier>,
    /// Non-modifier key keycode, if any (e.g., Space=49, F13=105)
    pub key: Option<i64>,
    /// Whether modifier is left (1) or right (2) side, maps keycode -> location
    pub modifier_locations: Vec<(i64, u32)>,
    /// Human-readable label (e.g., "Right ⌘" or "Ctrl+Space")
    pub label: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        #[cfg(target_os = "macos")]
        {
            // Default to Right Command key (keycode 54)
            Self {
                modifiers: vec![Modifier::Command],
                key: None,
                modifier_locations: vec![(54, 2)], // Right Command
                label: "Right ⌘".to_string(),
            }
        }
        #[cfg(target_os = "windows")]
        {
            // Default to Right Alt key (VK_RMENU = 165)
            Self {
                modifiers: vec![Modifier::Option],
                key: None,
                modifier_locations: vec![(165, 2)], // Right Alt
                label: "Right Alt".to_string(),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Self {
                modifiers: vec![Modifier::Command],
                key: None,
                modifier_locations: vec![(54, 2)],
                label: "Right ⌘".to_string(),
            }
        }
    }
}

impl HotkeyConfig {
    /// Get all keycodes that must be held for this hotkey
    pub fn required_keycodes(&self) -> HashSet<i64> {
        let mut keycodes: HashSet<i64> =
            self.modifier_locations.iter().map(|(kc, _)| *kc).collect();
        if let Some(key) = self.key {
            keycodes.insert(key);
        }
        keycodes
    }

    /// Get all modifier keycodes (for clearing before paste)
    pub fn modifier_keycodes(&self) -> Vec<i64> {
        self.modifier_locations.iter().map(|(kc, _)| *kc).collect()
    }
}

/// App mode for transcription routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    Local,
    ClientOnly,
    ServerOnly,
}

#[allow(clippy::derivable_impls)] // Platform-conditional default requires manual impl
impl Default for AppMode {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        {
            AppMode::ClientOnly
        }
        #[cfg(not(target_os = "windows"))]
        {
            AppMode::Local
        }
    }
}

/// App configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub hotkey: HotkeyConfig,
    #[serde(default)]
    pub mode: AppMode,
    #[serde(default)]
    pub server_url: Option<String>,
    #[serde(default = "default_server_port")]
    pub server_port: Option<u16>,
}

fn default_server_port() -> Option<u16> {
    Some(8765)
}

/// Get the config file path
fn config_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set")?;
        let config_dir = PathBuf::from(appdata).join("saytype");
        Ok(config_dir.join("config.json"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").map_err(|_| "HOME not set")?;
        let config_dir = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("com.raphaelmitas.saytype");
        Ok(config_dir.join("config.json"))
    }
}

/// Load configuration from disk
pub fn load_config() -> AppConfig {
    match config_path() {
        Ok(path) => {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(contents) => match serde_json::from_str(&contents) {
                        Ok(config) => {
                            println!("[CONFIG] Loaded config from {:?}", path);
                            return config;
                        }
                        Err(e) => {
                            eprintln!("[CONFIG] Failed to parse config: {}", e);
                        }
                    },
                    Err(e) => {
                        eprintln!("[CONFIG] Failed to read config: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("[CONFIG] Failed to get config path: {}", e);
        }
    }
    println!("[CONFIG] Using default config");
    AppConfig::default()
}

/// Save configuration to disk
pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = config_path()?;

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Failed to write config: {}", e))?;

    println!("[CONFIG] Saved config to {:?}", path);
    Ok(())
}

/// Map JavaScript event.code to platform keycode
#[cfg(not(target_os = "windows"))]
pub fn js_code_to_keycode(code: &str) -> Option<i64> {
    // macOS keycodes
    // Reference: https://eastmanreference.com/complete-list-of-applescript-key-codes
    match code {
        // Letters
        "KeyA" => Some(0),
        "KeyS" => Some(1),
        "KeyD" => Some(2),
        "KeyF" => Some(3),
        "KeyH" => Some(4),
        "KeyG" => Some(5),
        "KeyZ" => Some(6),
        "KeyX" => Some(7),
        "KeyC" => Some(8),
        "KeyV" => Some(9),
        "KeyB" => Some(11),
        "KeyQ" => Some(12),
        "KeyW" => Some(13),
        "KeyE" => Some(14),
        "KeyR" => Some(15),
        "KeyY" => Some(16),
        "KeyT" => Some(17),
        "Key1" | "Digit1" => Some(18),
        "Key2" | "Digit2" => Some(19),
        "Key3" | "Digit3" => Some(20),
        "Key4" | "Digit4" => Some(21),
        "Key6" | "Digit6" => Some(22),
        "Key5" | "Digit5" => Some(23),
        "Equal" => Some(24),
        "Key9" | "Digit9" => Some(25),
        "Key7" | "Digit7" => Some(26),
        "Minus" => Some(27),
        "Key8" | "Digit8" => Some(28),
        "Key0" | "Digit0" => Some(29),
        "BracketRight" => Some(30),
        "KeyO" => Some(31),
        "KeyU" => Some(32),
        "BracketLeft" => Some(33),
        "KeyI" => Some(34),
        "KeyP" => Some(35),
        "Enter" => Some(36),
        "KeyL" => Some(37),
        "KeyJ" => Some(38),
        "Quote" => Some(39),
        "KeyK" => Some(40),
        "Semicolon" => Some(41),
        "Backslash" => Some(42),
        "Comma" => Some(43),
        "Slash" => Some(44),
        "KeyN" => Some(45),
        "KeyM" => Some(46),
        "Period" => Some(47),
        "Tab" => Some(48),
        "Space" => Some(49),
        "Backquote" => Some(50),
        "Backspace" => Some(51),
        "Escape" => Some(53),

        // Modifier keys with side differentiation
        "MetaRight" => Some(54), // Right Command
        "MetaLeft" => Some(55),  // Left Command
        "ShiftLeft" => Some(56), // Left Shift
        "CapsLock" => Some(57),
        "AltLeft" => Some(58),      // Left Option
        "ControlLeft" => Some(59),  // Left Control
        "ShiftRight" => Some(60),   // Right Shift
        "AltRight" => Some(61),     // Right Option
        "ControlRight" => Some(62), // Right Control
        "Fn" => Some(63),           // Function key

        // Function keys
        "F17" => Some(64),
        "NumpadDecimal" => Some(65),
        "NumpadMultiply" => Some(67),
        "NumpadAdd" => Some(69),
        "NumLock" => Some(71),
        "NumpadDivide" => Some(75),
        "NumpadEnter" => Some(76),
        "NumpadSubtract" => Some(78),
        "F18" => Some(79),
        "F19" => Some(80),
        "NumpadEqual" => Some(81),
        "Numpad0" => Some(82),
        "Numpad1" => Some(83),
        "Numpad2" => Some(84),
        "Numpad3" => Some(85),
        "Numpad4" => Some(86),
        "Numpad5" => Some(87),
        "Numpad6" => Some(88),
        "Numpad7" => Some(89),
        "F20" => Some(90),
        "Numpad8" => Some(91),
        "Numpad9" => Some(92),
        "F5" => Some(96),
        "F6" => Some(97),
        "F7" => Some(98),
        "F3" => Some(99),
        "F8" => Some(100),
        "F9" => Some(101),
        "F11" => Some(103),
        "F13" => Some(105),
        "F16" => Some(106),
        "F14" => Some(107),
        "F10" => Some(109),
        "F12" => Some(111),
        "F15" => Some(113),
        "Home" => Some(115),
        "PageUp" => Some(116),
        "Delete" => Some(117),
        "F4" => Some(118),
        "End" => Some(119),
        "F2" => Some(120),
        "PageDown" => Some(121),
        "F1" => Some(122),
        "ArrowLeft" => Some(123),
        "ArrowRight" => Some(124),
        "ArrowDown" => Some(125),
        "ArrowUp" => Some(126),

        _ => None,
    }
}

/// Map JavaScript event.code to Windows Virtual Key code
#[cfg(target_os = "windows")]
pub fn js_code_to_keycode(code: &str) -> Option<i64> {
    match code {
        // Letters (VK_A=0x41 .. VK_Z=0x5A)
        "KeyA" => Some(0x41),
        "KeyB" => Some(0x42),
        "KeyC" => Some(0x43),
        "KeyD" => Some(0x44),
        "KeyE" => Some(0x45),
        "KeyF" => Some(0x46),
        "KeyG" => Some(0x47),
        "KeyH" => Some(0x48),
        "KeyI" => Some(0x49),
        "KeyJ" => Some(0x4A),
        "KeyK" => Some(0x4B),
        "KeyL" => Some(0x4C),
        "KeyM" => Some(0x4D),
        "KeyN" => Some(0x4E),
        "KeyO" => Some(0x4F),
        "KeyP" => Some(0x50),
        "KeyQ" => Some(0x51),
        "KeyR" => Some(0x52),
        "KeyS" => Some(0x53),
        "KeyT" => Some(0x54),
        "KeyU" => Some(0x55),
        "KeyV" => Some(0x56),
        "KeyW" => Some(0x57),
        "KeyX" => Some(0x58),
        "KeyY" => Some(0x59),
        "KeyZ" => Some(0x5A),

        // Numbers (VK_0=0x30 .. VK_9=0x39)
        "Key0" | "Digit0" => Some(0x30),
        "Key1" | "Digit1" => Some(0x31),
        "Key2" | "Digit2" => Some(0x32),
        "Key3" | "Digit3" => Some(0x33),
        "Key4" | "Digit4" => Some(0x34),
        "Key5" | "Digit5" => Some(0x35),
        "Key6" | "Digit6" => Some(0x36),
        "Key7" | "Digit7" => Some(0x37),
        "Key8" | "Digit8" => Some(0x38),
        "Key9" | "Digit9" => Some(0x39),

        // Common keys
        "Enter" => Some(0x0D),     // VK_RETURN
        "Tab" => Some(0x09),       // VK_TAB
        "Space" => Some(0x20),     // VK_SPACE
        "Backspace" => Some(0x08), // VK_BACK
        "Escape" => Some(0x1B),    // VK_ESCAPE
        "Delete" => Some(0x2E),    // VK_DELETE

        // Modifiers with side differentiation (using extended VK codes)
        "ShiftLeft" => Some(0xA0),    // VK_LSHIFT
        "ShiftRight" => Some(0xA1),   // VK_RSHIFT
        "ControlLeft" => Some(0xA2),  // VK_LCONTROL
        "ControlRight" => Some(0xA3), // VK_RCONTROL
        "AltLeft" => Some(0xA4),      // VK_LMENU
        "AltRight" => Some(0xA5),     // VK_RMENU
        "MetaLeft" => Some(0x5B),     // VK_LWIN
        "MetaRight" => Some(0x5C),    // VK_RWIN
        "CapsLock" => Some(0x14),     // VK_CAPITAL

        // Function keys
        "F1" => Some(0x70),
        "F2" => Some(0x71),
        "F3" => Some(0x72),
        "F4" => Some(0x73),
        "F5" => Some(0x74),
        "F6" => Some(0x75),
        "F7" => Some(0x76),
        "F8" => Some(0x77),
        "F9" => Some(0x78),
        "F10" => Some(0x79),
        "F11" => Some(0x7A),
        "F12" => Some(0x7B),
        "F13" => Some(0x7C),
        "F14" => Some(0x7D),
        "F15" => Some(0x7E),
        "F16" => Some(0x7F),
        "F17" => Some(0x80),
        "F18" => Some(0x81),
        "F19" => Some(0x82),
        "F20" => Some(0x83),

        // Arrow keys
        "ArrowLeft" => Some(0x25),
        "ArrowUp" => Some(0x26),
        "ArrowRight" => Some(0x27),
        "ArrowDown" => Some(0x28),

        // Navigation
        "Home" => Some(0x24),
        "End" => Some(0x23),
        "PageUp" => Some(0x21),
        "PageDown" => Some(0x22),

        // Punctuation / symbols
        "Equal" => Some(0xBB),        // VK_OEM_PLUS
        "Minus" => Some(0xBD),        // VK_OEM_MINUS
        "BracketLeft" => Some(0xDB),  // VK_OEM_4
        "BracketRight" => Some(0xDD), // VK_OEM_6
        "Backslash" => Some(0xDC),    // VK_OEM_5
        "Semicolon" => Some(0xBA),    // VK_OEM_1
        "Quote" => Some(0xDE),        // VK_OEM_7
        "Comma" => Some(0xBC),        // VK_OEM_COMMA
        "Period" => Some(0xBE),       // VK_OEM_PERIOD
        "Slash" => Some(0xBF),        // VK_OEM_2
        "Backquote" => Some(0xC0),    // VK_OEM_3

        // Numpad
        "Numpad0" => Some(0x60),
        "Numpad1" => Some(0x61),
        "Numpad2" => Some(0x62),
        "Numpad3" => Some(0x63),
        "Numpad4" => Some(0x64),
        "Numpad5" => Some(0x65),
        "Numpad6" => Some(0x66),
        "Numpad7" => Some(0x67),
        "Numpad8" => Some(0x68),
        "Numpad9" => Some(0x69),
        "NumpadMultiply" => Some(0x6A),
        "NumpadAdd" => Some(0x6B),
        "NumpadSubtract" => Some(0x6D),
        "NumpadDecimal" => Some(0x6E),
        "NumpadDivide" => Some(0x6F),
        "NumpadEnter" => Some(0x0D),
        "NumLock" => Some(0x90),

        _ => None,
    }
}

/// Map platform keycode to display label
#[cfg(target_os = "windows")]
pub fn keycode_to_label(keycode: i64) -> String {
    match keycode {
        // Modifiers
        0xA0 => "Left Shift".to_string(),
        0xA1 => "Right Shift".to_string(),
        0xA2 => "Left Ctrl".to_string(),
        0xA3 => "Right Ctrl".to_string(),
        0xA4 => "Left Alt".to_string(),
        0xA5 => "Right Alt".to_string(),
        0x5B => "Left Win".to_string(),
        0x5C => "Right Win".to_string(),
        0x14 => "Caps Lock".to_string(),

        // Common keys
        0x20 => "Space".to_string(),
        0x0D => "Enter".to_string(),
        0x09 => "Tab".to_string(),
        0x1B => "Escape".to_string(),
        0x08 => "Backspace".to_string(),
        0x2E => "Delete".to_string(),

        // Function keys
        k @ 0x70..=0x83 => format!("F{}", k - 0x70 + 1),

        // Letters
        k @ 0x41..=0x5A => format!("{}", (k as u8) as char),

        // Numbers
        k @ 0x30..=0x39 => format!("{}", k - 0x30),

        // Arrow keys
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),

        // Navigation
        0x24 => "Home".to_string(),
        0x23 => "End".to_string(),
        0x21 => "Page Up".to_string(),
        0x22 => "Page Down".to_string(),

        _ => format!("VK{}", keycode),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn keycode_to_label(keycode: i64) -> String {
    match keycode {
        // Modifiers
        54 => "Right ⌘".to_string(),
        55 => "Left ⌘".to_string(),
        56 => "Left ⇧".to_string(),
        60 => "Right ⇧".to_string(),
        58 => "Left ⌥".to_string(),
        61 => "Right ⌥".to_string(),
        59 => "Left ⌃".to_string(),
        62 => "Right ⌃".to_string(),
        57 => "⇪ Caps Lock".to_string(),
        63 => "fn".to_string(),

        // Common keys
        49 => "Space".to_string(),
        36 => "Return".to_string(),
        48 => "Tab".to_string(),
        53 => "Escape".to_string(),
        51 => "Delete".to_string(),

        // Function keys
        122 => "F1".to_string(),
        120 => "F2".to_string(),
        99 => "F3".to_string(),
        118 => "F4".to_string(),
        96 => "F5".to_string(),
        97 => "F6".to_string(),
        98 => "F7".to_string(),
        100 => "F8".to_string(),
        101 => "F9".to_string(),
        109 => "F10".to_string(),
        103 => "F11".to_string(),
        111 => "F12".to_string(),
        105 => "F13".to_string(),
        107 => "F14".to_string(),
        113 => "F15".to_string(),
        106 => "F16".to_string(),
        64 => "F17".to_string(),
        79 => "F18".to_string(),
        80 => "F19".to_string(),
        90 => "F20".to_string(),

        // Letters
        0 => "A".to_string(),
        11 => "B".to_string(),
        8 => "C".to_string(),
        2 => "D".to_string(),
        14 => "E".to_string(),
        3 => "F".to_string(),
        5 => "G".to_string(),
        4 => "H".to_string(),
        34 => "I".to_string(),
        38 => "J".to_string(),
        40 => "K".to_string(),
        37 => "L".to_string(),
        46 => "M".to_string(),
        45 => "N".to_string(),
        31 => "O".to_string(),
        35 => "P".to_string(),
        12 => "Q".to_string(),
        15 => "R".to_string(),
        1 => "S".to_string(),
        17 => "T".to_string(),
        32 => "U".to_string(),
        9 => "V".to_string(),
        13 => "W".to_string(),
        7 => "X".to_string(),
        16 => "Y".to_string(),
        6 => "Z".to_string(),

        // Numbers
        18 => "1".to_string(),
        19 => "2".to_string(),
        20 => "3".to_string(),
        21 => "4".to_string(),
        23 => "5".to_string(),
        22 => "6".to_string(),
        26 => "7".to_string(),
        28 => "8".to_string(),
        25 => "9".to_string(),
        29 => "0".to_string(),

        // Arrow keys
        123 => "←".to_string(),
        124 => "→".to_string(),
        125 => "↓".to_string(),
        126 => "↑".to_string(),

        _ => format!("Key{}", keycode),
    }
}

/// Check if a keycode is a modifier key
pub fn is_modifier_keycode(keycode: i64) -> bool {
    #[cfg(target_os = "windows")]
    {
        // VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN, VK_CAPITAL
        matches!(
            keycode,
            0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5 | 0x5B | 0x5C | 0x14
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        matches!(keycode, 54..=63)
    }
}

/// Build a display label from keycodes
pub fn build_label(keycodes: &[i64]) -> String {
    let mut labels: Vec<String> = keycodes.iter().map(|&kc| keycode_to_label(kc)).collect();

    // Sort so modifiers come first
    labels.sort_by(|a, b| {
        let a_is_mod = is_modifier_label(a);
        let b_is_mod = is_modifier_label(b);
        b_is_mod.cmp(&a_is_mod)
    });

    labels.join("+")
}

fn is_modifier_label(label: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        label.contains("Ctrl")
            || label.contains("Alt")
            || label.contains("Shift")
            || label.contains("Win")
            || label.contains("Caps")
    }
    #[cfg(not(target_os = "windows"))]
    {
        label.contains('⌘')
            || label.contains('⇧')
            || label.contains('⌥')
            || label.contains('⌃')
            || label.contains("fn")
            || label.contains("Caps")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_default_config() {
        let config = HotkeyConfig::default();
        assert_eq!(config.label, "Right ⌘");
        assert!(config.required_keycodes().contains(&54));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_default_config_windows() {
        let config = HotkeyConfig::default();
        assert_eq!(config.label, "Right Alt");
        assert!(config.required_keycodes().contains(&165));
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_keycode_mapping() {
        assert_eq!(js_code_to_keycode("MetaRight"), Some(54));
        assert_eq!(js_code_to_keycode("Space"), Some(49));
        assert_eq!(js_code_to_keycode("F13"), Some(105));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_keycode_mapping_windows() {
        assert_eq!(js_code_to_keycode("AltRight"), Some(0xA5));
        assert_eq!(js_code_to_keycode("Space"), Some(0x20));
        assert_eq!(js_code_to_keycode("F13"), Some(0x7C));
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_label_building() {
        assert_eq!(build_label(&[54]), "Right ⌘");
        assert_eq!(build_label(&[59, 49]), "Left ⌃+Space");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_label_building_windows() {
        assert_eq!(build_label(&[0xA5]), "Right Alt");
        assert_eq!(build_label(&[0xA2, 0x20]), "Left Ctrl+Space");
    }
}
