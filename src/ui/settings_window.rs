use crate::app_state::{AppEvent, SharedState};
use crate::settings::{HotkeyBinding, Modifier, Settings, VirtualKey};
use crate::ui::theme::HideIconsTheme;
use gpui::*;
use std::sync::mpsc;

pub struct SettingsView {
    pub settings: Settings,
    pub hotkey_conflict: bool,
    pub taskbar_hotkey_conflict: bool,
    pub capturing_hotkey: bool,
    pub capturing_taskbar_hotkey: bool,
    hotkey_focus: FocusHandle,
    taskbar_hotkey_focus: FocusHandle,
    state: SharedState,
    #[allow(dead_code)]
    event_tx: mpsc::Sender<AppEvent>,
}

impl SettingsView {
    pub fn new(
        settings: Settings,
        event_tx: mpsc::Sender<AppEvent>,
        state: SharedState,
        cx: &mut App,
    ) -> Self {
        Self {
            settings,
            hotkey_conflict: false,
            taskbar_hotkey_conflict: false,
            capturing_hotkey: false,
            capturing_taskbar_hotkey: false,
            hotkey_focus: cx.focus_handle(),
            taskbar_hotkey_focus: cx.focus_handle(),
            state,
            event_tx,
        }
    }

    fn format_hotkey(binding: &HotkeyBinding) -> String {
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

    /// Try to parse a GPUI Keystroke into our HotkeyBinding.
    /// Returns None if only modifiers were pressed (no primary key).
    fn parse_keystroke(keystroke: &Keystroke) -> Option<HotkeyBinding> {
        let key_str = keystroke.key.as_str();

        // Skip if only modifier keys were pressed
        if matches!(key_str, "control" | "alt" | "shift" | "platform" | "fn") {
            return None;
        }

        let mut modifiers = Vec::new();
        if keystroke.modifiers.control {
            modifiers.push(Modifier::Ctrl);
        }
        if keystroke.modifiers.alt {
            modifiers.push(Modifier::Alt);
        }
        if keystroke.modifiers.shift {
            modifiers.push(Modifier::Shift);
        }
        if keystroke.modifiers.platform {
            modifiers.push(Modifier::Win);
        }

        // Must have at least one modifier for a valid hotkey
        if modifiers.is_empty() {
            return None;
        }

        // Parse the primary key
        let key = if key_str.len() == 1 {
            let c = key_str.chars().next().unwrap();
            if c.is_ascii_alphanumeric() {
                VirtualKey::Char(c.to_ascii_uppercase())
            } else {
                return None;
            }
        } else if let Some(n) = key_str.strip_prefix('f') {
            if let Ok(num) = n.parse::<u8>() {
                if (1..=24).contains(&num) {
                    VirtualKey::F(num)
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            return None;
        };

        let binding = HotkeyBinding { modifiers, key };
        if binding.is_valid() {
            Some(binding)
        } else {
            None
        }
    }

    /// Persists settings to disk, syncs shared state, updates startup, and
    /// re-registers hotkeys on the WinAPI thread.
    fn apply_settings(&mut self) {
        let _ = self.settings.save();
        crate::startup::set_startup(self.settings.start_with_windows);

        let mut guard = self.state.lock().unwrap();
        guard.settings = self.settings.clone();
        let winapi_hwnd = guard.winapi_hwnd;
        drop(guard);

        // Re-register hotkeys on the WinAPI thread
        if let Some(raw_hwnd) = winapi_hwnd {
            let hwnd = windows::Win32::Foundation::HWND(raw_hwnd as *mut _);
            // SAFETY: Posting a custom message to the WinAPI thread to re-register hotkeys.
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                    Some(hwnd),
                    crate::winapi_thread::WM_REREGISTER_HOTKEY,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0),
                );
            }
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hotkey_text = if self.capturing_hotkey {
            "Press a key combination...".to_string()
        } else {
            Self::format_hotkey(&self.settings.hotkey)
        };

        let conflict_warning = if self.hotkey_conflict {
            Some(
                div()
                    .text_color(HideIconsTheme::primary())
                    .text_sm()
                    .child("This hotkey is already in use by another application."),
            )
        } else {
            None
        };

        let hotkey_border = if self.capturing_hotkey {
            HideIconsTheme::primary()
        } else {
            HideIconsTheme::border()
        };

        let start_with_windows = self.settings.start_with_windows;
        let startup_bg = if start_with_windows {
            HideIconsTheme::primary()
        } else {
            HideIconsTheme::border()
        };
        let startup_offset = if start_with_windows {
            px(18.0)
        } else {
            px(2.0)
        };

        div()
            .size_full()
            .bg(HideIconsTheme::background())
            .text_color(HideIconsTheme::text_primary())
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            // Tip alert
            .child({
                let tip = if self.settings.hide_taskbar_with_icons {
                    "Double-click on your desktop to toggle visibility of icons and taskbar."
                } else {
                    "Double-click on your desktop to toggle visibility of icons."
                };
                div()
                    .px_3()
                    .py_2()
                    .rounded_sm()
                    .border_1()
                    .border_color(HideIconsTheme::border())
                    .bg(HideIconsTheme::surface())
                    .text_color(HideIconsTheme::primary())
                    .text_sm()
                    .child(tip)
            })
            // Hotkey section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_color(HideIconsTheme::text_secondary())
                            .text_sm()
                            .child("Hotkey"),
                    )
                    .child(
                        div()
                            .id("hotkey-capture")
                            .track_focus(&self.hotkey_focus)
                            .px_3()
                            .py_2()
                            .rounded_sm()
                            .border_1()
                            .border_color(hotkey_border)
                            .bg(HideIconsTheme::surface())
                            .cursor_pointer()
                            .child(hotkey_text)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, _cx| {
                                    this.capturing_hotkey = true;
                                    this.hotkey_conflict = false;
                                    window.focus(&this.hotkey_focus);
                                }),
                            )
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                                if !this.capturing_hotkey {
                                    return;
                                }
                                if let Some(binding) =
                                    SettingsView::parse_keystroke(&event.keystroke)
                                {
                                    this.settings.hotkey = binding;
                                    this.capturing_hotkey = false;
                                    this.apply_settings();
                                    cx.notify();
                                }
                            })),
                    )
                    .children(conflict_warning),
            )
            // Startup section
            .child(
                div()
                    .id("startup-toggle")
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            this.settings.start_with_windows = !this.settings.start_with_windows;
                            this.apply_settings();
                        }),
                    )
                    .child(
                        div()
                            .w(px(36.0))
                            .h(px(20.0))
                            .rounded(px(10.0))
                            .bg(startup_bg)
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .ml(startup_offset)
                                    .size(px(16.0))
                                    .rounded(px(8.0))
                                    .bg(gpui::rgb(0xffffff)),
                            ),
                    )
                    .child("Start with Windows"),
            )
            // Taskbar section
            .child({
                let hide_taskbar = self.settings.hide_taskbar_with_icons;

                let taskbar_bg = if hide_taskbar {
                    HideIconsTheme::primary()
                } else {
                    HideIconsTheme::border()
                };
                let taskbar_offset = if hide_taskbar { px(18.0) } else { px(2.0) };

                let mut taskbar_section = div().flex().flex_col().gap_2().child(
                    div()
                        .id("taskbar-toggle")
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _window, _cx| {
                                this.settings.hide_taskbar_with_icons =
                                    !this.settings.hide_taskbar_with_icons;
                                this.apply_settings();
                            }),
                        )
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(20.0))
                                .rounded(px(10.0))
                                .bg(taskbar_bg)
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .ml(taskbar_offset)
                                        .size(px(16.0))
                                        .rounded(px(8.0))
                                        .bg(gpui::rgb(0xffffff)),
                                ),
                        )
                        .child("Double-click hides taskbar"),
                );

                // When "Also hide taskbar" is off, show a separate taskbar hotkey input
                if !hide_taskbar {
                    let tb_hotkey_text = if self.capturing_taskbar_hotkey {
                        "Press a key combination...".to_string()
                    } else if let Some(ref binding) = self.settings.taskbar_hotkey {
                        Self::format_hotkey(binding)
                    } else {
                        "Click to set taskbar hotkey".to_string()
                    };

                    let tb_hotkey_border = if self.capturing_taskbar_hotkey {
                        HideIconsTheme::primary()
                    } else {
                        HideIconsTheme::border()
                    };

                    let tb_conflict = if self.taskbar_hotkey_conflict {
                        Some(
                            div()
                                .text_color(HideIconsTheme::primary())
                                .text_sm()
                                .child("This hotkey conflicts with another binding."),
                        )
                    } else {
                        None
                    };

                    taskbar_section = taskbar_section
                        .child(
                            div()
                                .text_color(HideIconsTheme::text_secondary())
                                .text_sm()
                                .child("Taskbar hide hotkey"),
                        )
                        .child(
                            div()
                                .id("taskbar-hotkey-capture")
                                .track_focus(&self.taskbar_hotkey_focus)
                                .px_3()
                                .py_2()
                                .rounded_sm()
                                .border_1()
                                .border_color(tb_hotkey_border)
                                .bg(HideIconsTheme::surface())
                                .cursor_pointer()
                                .child(tb_hotkey_text)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, window, _cx| {
                                        this.capturing_taskbar_hotkey = true;
                                        this.taskbar_hotkey_conflict = false;
                                        window.focus(&this.taskbar_hotkey_focus);
                                    }),
                                )
                                .on_key_down(cx.listener(
                                    |this, event: &KeyDownEvent, _window, cx| {
                                        if !this.capturing_taskbar_hotkey {
                                            return;
                                        }
                                        if let Some(binding) =
                                            SettingsView::parse_keystroke(&event.keystroke)
                                        {
                                            this.capturing_taskbar_hotkey = false;
                                            // Validate: must differ from icon hotkey
                                            if binding == this.settings.hotkey {
                                                this.taskbar_hotkey_conflict = true;
                                                cx.notify();
                                                return;
                                            }
                                            this.settings.taskbar_hotkey = Some(binding);
                                            this.apply_settings();
                                            cx.notify();
                                        }
                                    },
                                )),
                        )
                        .children(tb_conflict);
                }

                taskbar_section
            })
    }
}
