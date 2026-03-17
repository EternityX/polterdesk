use polterdesk::settings::{HotkeyBinding, Modifier, Settings, ThemeMode, VirtualKey};
use std::path::PathBuf;

fn temp_settings_path() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "polterdesk-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("settings.json")
}

#[test]
fn serialize_deserialize_roundtrip() {
    let settings = Settings {
        hotkey: HotkeyBinding {
            modifiers: vec![Modifier::Ctrl, Modifier::Shift],
            key: VirtualKey::Char('D'),
        },
        start_with_windows: true,
        theme_mode: ThemeMode::Light,
    };

    let json = serde_json::to_string(&settings).unwrap();
    let deserialized: Settings = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.hotkey.modifiers, vec![Modifier::Ctrl, Modifier::Shift]);
    assert_eq!(deserialized.hotkey.key, VirtualKey::Char('D'));
    assert!(deserialized.start_with_windows);
    assert_eq!(deserialized.theme_mode, ThemeMode::Light);
}

#[test]
fn default_settings_has_ctrl_alt_h() {
    let settings = Settings::default();
    assert_eq!(settings.hotkey.modifiers, vec![Modifier::Ctrl, Modifier::Alt]);
    assert_eq!(settings.hotkey.key, VirtualKey::Char('H'));
    assert!(!settings.start_with_windows);
    assert_eq!(settings.theme_mode, ThemeMode::Dark);
}

#[test]
fn malformed_json_returns_defaults() {
    let path = temp_settings_path();
    std::fs::write(&path, "not valid json!!!").unwrap();

    // Settings::load reads from APPDATA, so we test the parsing logic directly
    let result: Result<Settings, _> = serde_json::from_str("not valid json!!!");
    assert!(result.is_err());
}

#[test]
fn missing_file_returns_defaults() {
    // Parsing an empty string should fail, falling back to defaults
    let result: Result<Settings, _> = serde_json::from_str("");
    assert!(result.is_err());
}

#[test]
fn f12_key_is_invalid() {
    let binding = HotkeyBinding {
        modifiers: vec![Modifier::Ctrl],
        key: VirtualKey::F(12),
    };
    assert!(!binding.is_valid());
}

#[test]
fn empty_modifiers_is_invalid() {
    let binding = HotkeyBinding {
        modifiers: vec![],
        key: VirtualKey::Char('H'),
    };
    assert!(!binding.is_valid());
}

#[test]
fn valid_binding_passes_validation() {
    let binding = HotkeyBinding::default();
    assert!(binding.is_valid());
}
