use crate::settings::{HotkeyBinding, Modifier, VirtualKey};
use crate::ui::theme::HideIconsTheme;
use gpui::*;

#[allow(dead_code)]
pub struct HotkeyInput {
    pub binding: Option<HotkeyBinding>,
    pub pending: Option<HotkeyBinding>,
    pub conflict: bool,
    pub focused: bool,
}

#[allow(dead_code)]
impl HotkeyInput {
    pub fn new(binding: Option<HotkeyBinding>) -> Self {
        Self {
            binding,
            pending: None,
            conflict: false,
            focused: false,
        }
    }

    pub fn current_binding(&self) -> Option<&HotkeyBinding> {
        self.pending.as_ref().or(self.binding.as_ref())
    }

    fn display_text(&self) -> String {
        if let Some(binding) = self.current_binding() {
            format_hotkey(binding)
        } else {
            "Press a key combination...".to_string()
        }
    }

    pub fn clear(&mut self) {
        self.pending = None;
        self.binding = None;
        self.conflict = false;
    }
}

#[allow(dead_code)]
pub fn format_hotkey(binding: &HotkeyBinding) -> String {
    let mut parts: Vec<String> = Vec::new();
    for m in &binding.modifiers {
        parts.push(
            match m {
                Modifier::Ctrl => "Ctrl",
                Modifier::Alt => "Alt",
                Modifier::Shift => "Shift",
                Modifier::Win => "Win",
            }
            .to_string(),
        );
    }
    match &binding.key {
        VirtualKey::Char(c) => parts.push(c.to_uppercase().to_string()),
        VirtualKey::F(n) => parts.push(format!("F{n}")),
    }
    parts.join(" + ")
}

impl Render for HotkeyInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.display_text();
        let is_placeholder = self.binding.is_none() && self.pending.is_none();
        let text_color = if is_placeholder {
            HideIconsTheme::text_secondary()
        } else {
            HideIconsTheme::text_primary()
        };

        div()
            .id("hotkey-input")
            .px_3()
            .py_2()
            .rounded_sm()
            .border_1()
            .border_color(HideIconsTheme::border())
            .bg(HideIconsTheme::surface())
            .text_color(text_color)
            .child(text)
    }
}
