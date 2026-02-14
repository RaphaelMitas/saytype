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
        // Default to Right Command key
        Self {
            modifiers: vec![Modifier::Command],
            key: None,
            modifier_locations: vec![(54, 2)], // Right Command
            label: "Right ⌘".to_string(),
        }
    }
}

impl HotkeyConfig {
    /// Get all keycodes that must be held for this hotkey
    pub fn required_keycodes(&self) -> HashSet<i64> {
        let mut keycodes: HashSet<i64> = self.modifier_locations.iter().map(|(kc, _)| *kc).collect();
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

/// App configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub hotkey: HotkeyConfig,
}

/// Get the config file path
fn config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set")?;
    let config_dir = PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("com.raphaelmitas.saytype");
    Ok(config_dir.join("config.json"))
}

/// Load configuration from disk
pub fn load_config() -> AppConfig {
    match config_path() {
        Ok(path) => {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(contents) => {
                        match serde_json::from_str(&contents) {
                            Ok(config) => {
                                println!("[CONFIG] Loaded config from {:?}", path);
                                return config;
                            }
                            Err(e) => {
                                eprintln!("[CONFIG] Failed to parse config: {}", e);
                            }
                        }
                    }
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

    fs::write(&path, json)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    println!("[CONFIG] Saved config to {:?}", path);
    Ok(())
}

/// Map JavaScript event.code to macOS keycode
pub fn js_code_to_keycode(code: &str) -> Option<i64> {
    // Common keycodes for macOS
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
        "MetaRight" => Some(54),   // Right Command
        "MetaLeft" => Some(55),    // Left Command
        "ShiftLeft" => Some(56),   // Left Shift
        "CapsLock" => Some(57),
        "AltLeft" => Some(58),     // Left Option
        "ControlLeft" => Some(59), // Left Control
        "ShiftRight" => Some(60),  // Right Shift
        "AltRight" => Some(61),    // Right Option
        "ControlRight" => Some(62),// Right Control
        "Fn" => Some(63),          // Function key

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

/// Map macOS keycode to display label
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
    matches!(keycode, 54 | 55 | 56 | 60 | 58 | 61 | 59 | 62 | 57 | 63)
}

/// Build a display label from keycodes
pub fn build_label(keycodes: &[i64]) -> String {
    let mut labels: Vec<String> = keycodes.iter().map(|&kc| keycode_to_label(kc)).collect();

    // Sort so modifiers come first
    labels.sort_by(|a, b| {
        let a_is_mod = a.contains('⌘') || a.contains('⇧') || a.contains('⌥') || a.contains('⌃') || a.contains("fn") || a.contains("Caps");
        let b_is_mod = b.contains('⌘') || b.contains('⇧') || b.contains('⌥') || b.contains('⌃') || b.contains("fn") || b.contains("Caps");
        b_is_mod.cmp(&a_is_mod)
    });

    labels.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HotkeyConfig::default();
        assert_eq!(config.label, "Right ⌘");
        assert!(config.required_keycodes().contains(&54));
    }

    #[test]
    fn test_keycode_mapping() {
        assert_eq!(js_code_to_keycode("MetaRight"), Some(54));
        assert_eq!(js_code_to_keycode("Space"), Some(49));
        assert_eq!(js_code_to_keycode("F13"), Some(105));
    }

    #[test]
    fn test_label_building() {
        assert_eq!(build_label(&[54]), "Right ⌘");
        assert_eq!(build_label(&[59, 49]), "Left ⌃+Space");
    }
}
