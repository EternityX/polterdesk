use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Keyboard modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Win,
}

/// Virtual key for the hotkey binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VirtualKey {
    Char(char),
    F(u8),
}

impl VirtualKey {
    /// Returns true if this key is F12 (prohibited).
    pub fn is_f12(&self) -> bool {
        matches!(self, VirtualKey::F(12))
    }
}

/// A hotkey binding consisting of modifiers and a primary key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyBinding {
    pub modifiers: Vec<Modifier>,
    pub key: VirtualKey,
}

impl Default for HotkeyBinding {
    fn default() -> Self {
        Self {
            modifiers: vec![Modifier::Ctrl, Modifier::Alt],
            key: VirtualKey::Char('H'),
        }
    }
}

impl HotkeyBinding {
    /// Validates the binding. Returns false if F12 or no modifiers.
    pub fn is_valid(&self) -> bool {
        !self.modifiers.is_empty() && !self.key.is_f12()
    }
}

/// Behavior when the settings window minimize button is clicked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinimizeBehavior {
    Taskbar,
    #[default]
    Tray,
}

/// Behavior when the settings window close button is clicked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseBehavior {
    Exit,
    #[default]
    Tray,
}

/// UI theme mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

/// User settings persisted to %APPDATA%\Polterdesk\settings.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_hotkey")]
    pub hotkey: Option<HotkeyBinding>,
    pub start_with_windows: bool,
    pub theme_mode: ThemeMode,
    /// When true, all toggle actions (hotkey + double-click) also toggle taskbar auto-hide.
    #[serde(default = "default_true")]
    pub hide_taskbar_with_icons: bool,
    /// Separate hotkey for taskbar toggle; active only when hide_taskbar_with_icons is false.
    #[serde(default)]
    pub taskbar_hotkey: Option<HotkeyBinding>,
    /// When true, the app starts hidden in the system tray (no settings window shown).
    #[serde(default)]
    pub start_minimized: bool,
    /// What happens when the minimize button is clicked.
    #[serde(default)]
    pub minimize_behavior: MinimizeBehavior,
    /// What happens when the close button is clicked.
    #[serde(default)]
    pub close_behavior: CloseBehavior,
    /// Persisted original taskbar state for crash recovery; cleared after restore.
    #[serde(default)]
    pub taskbar_original_state: Option<u32>,
}

fn default_true() -> bool {
    true
}

fn default_hotkey() -> Option<HotkeyBinding> {
    Some(HotkeyBinding::default())
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: Some(HotkeyBinding::default()),
            start_with_windows: false,
            theme_mode: ThemeMode::Dark,
            hide_taskbar_with_icons: true,
            start_minimized: false,
            minimize_behavior: MinimizeBehavior::Tray,
            close_behavior: CloseBehavior::Tray,
            taskbar_hotkey: None,
            taskbar_original_state: None,
        }
    }
}

impl Settings {
    /// Returns the path to the settings file.
    fn settings_path() -> Option<PathBuf> {
        std::env::var("APPDATA").ok().map(|appdata| {
            PathBuf::from(appdata)
                .join("Polterdesk")
                .join("settings.json")
        })
    }

    /// Loads settings from disk, falling back to defaults on any error.
    pub fn load() -> Self {
        let Some(path) = Self::settings_path() else {
            return Self::default();
        };

        let Ok(contents) = std::fs::read_to_string(&path) else {
            let settings = Self::default();
            let _ = settings.save();
            return settings;
        };

        match serde_json::from_str::<Settings>(&contents) {
            Ok(mut settings) => {
                // Validate hotkey - revert to default if invalid
                if let Some(ref hk) = settings.hotkey {
                    if !hk.is_valid() {
                        eprintln!("Invalid hotkey in settings, reverting to default");
                        settings.hotkey = Some(HotkeyBinding::default());
                        let _ = settings.save();
                    }
                }
                settings
            }
            Err(e) => {
                eprintln!("Failed to parse settings: {e}, using defaults");
                let settings = Self::default();
                let _ = settings.save();
                settings
            }
        }
    }

    /// Saves settings to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::settings_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "APPDATA not set",
            ));
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&path, json)
    }
}
