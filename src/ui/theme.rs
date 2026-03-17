use gpui::rgb;
use gpui_component::{Theme, ThemeSet};
use std::rc::Rc;

#[allow(dead_code)]
const EVERFOREST_THEME: &str = include_str!("../../themes/everforest.json");
const CATPPUCCIN_THEME: &str = include_str!("../../themes/catppuccin.json");

/// Catppuccin Mocha color tokens for custom UI elements.
pub struct HideIconsTheme;

impl HideIconsTheme {
    /// Background: Catppuccin Mocha mantle #181825
    pub fn background() -> gpui::Hsla {
        rgb(0x181825).into()
    }

    /// Surface: Catppuccin Mocha surface0 #302d41
    pub fn surface() -> gpui::Hsla {
        rgb(0x302d41).into()
    }

    /// Border: Catppuccin Mocha surface1 #313244
    pub fn border() -> gpui::Hsla {
        rgb(0x313244).into()
    }

    /// Primary accent: Catppuccin Mocha blue #89b4fa
    pub fn primary() -> gpui::Hsla {
        rgb(0x89b4fa).into()
    }

    /// Primary hover: Catppuccin Mocha sapphire #74c7ec
    #[allow(dead_code)]
    pub fn primary_hover() -> gpui::Hsla {
        rgb(0x74c7ec).into()
    }

    /// Text primary: Catppuccin Mocha text #cdd6f4
    pub fn text_primary() -> gpui::Hsla {
        rgb(0xcdd6f4).into()
    }

    /// Text secondary: Catppuccin Mocha overlay0 #6c7086
    pub fn text_secondary() -> gpui::Hsla {
        rgb(0x6c7086).into()
    }
}

/// Initialize gpui-component and apply the Catppuccin Mocha theme.
pub fn apply_theme(cx: &mut gpui::App) {
    gpui_component::init(cx);

    // Parse and apply the Catppuccin Mocha theme
    if let Ok(theme_set) = serde_json::from_str::<ThemeSet>(CATPPUCCIN_THEME) {
        for config in &theme_set.themes {
            if config.name.as_ref() == "Catppuccin Mocha" {
                let rc_config = Rc::new(config.clone());
                let theme = Theme::global_mut(cx);
                theme.dark_theme = rc_config.clone();
                theme.apply_config(&rc_config);
                break;
            }
        }
    }
}
