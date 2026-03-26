use crate::app_state::{AppEvent, SharedState};
use crate::settings::{
    CloseBehavior, HotkeyBinding, MinimizeBehavior, Modifier, Settings, VirtualKey,
};
use crate::ui::theme::HideIconsTheme;
use gpui::*;
use gpui_component::link::Link;
use gpui_component::radio::RadioGroup;
use gpui_component::scroll::ScrollableElement;
use gpui_component::tab::{Tab, TabBar};
use std::sync::mpsc;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Icons,
    Taskbar,
    System,
    About,
}

const TAB_ORDER: [SettingsTab; 4] = [
    SettingsTab::Icons,
    SettingsTab::Taskbar,
    SettingsTab::System,
    SettingsTab::About,
];

pub struct SettingsView {
    pub settings: Settings,
    pub hotkey_conflict: bool,
    pub taskbar_hotkey_conflict: bool,
    pub capturing_hotkey: bool,
    pub capturing_taskbar_hotkey: bool,
    hotkey_focus: FocusHandle,
    taskbar_hotkey_focus: FocusHandle,
    active_tab: SettingsTab,
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
            active_tab: SettingsTab::Icons,
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

    /// Renders a toggle switch with a label.
    fn render_toggle(
        id: &str,
        enabled: bool,
        label: &str,
        on_click: impl Fn(&mut Self, &MouseDownEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let bg = if enabled {
            HideIconsTheme::primary()
        } else {
            HideIconsTheme::border()
        };
        let offset = if enabled { px(18.0) } else { px(2.0) };

        div()
            .id(SharedString::from(id.to_string()))
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(on_click))
            .child(
                div()
                    .w(px(36.0))
                    .h(px(20.0))
                    .rounded(px(10.0))
                    .bg(bg)
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .ml(offset)
                            .size(px(16.0))
                            .rounded(px(8.0))
                            .bg(gpui::rgb(0xffffff)),
                    ),
            )
            .child(SharedString::from(label.to_string()))
    }

    fn render_icons_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let hotkey_text = if self.capturing_hotkey {
            "Press a key combination... (Esc to clear)".to_string()
        } else if let Some(ref binding) = self.settings.hotkey {
            Self::format_hotkey(binding)
        } else {
            "Click to set hotkey".to_string()
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

        div()
            .flex()
            .flex_col()
            .gap_4()
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
                                if event.keystroke.key.as_str() == "escape" {
                                    this.settings.hotkey = None;
                                    this.capturing_hotkey = false;
                                    this.apply_settings();
                                    cx.notify();
                                    return;
                                }
                                if let Some(binding) =
                                    SettingsView::parse_keystroke(&event.keystroke)
                                {
                                    this.settings.hotkey = Some(binding);
                                    this.capturing_hotkey = false;
                                    this.apply_settings();
                                    cx.notify();
                                }
                            })),
                    )
                    .children(conflict_warning),
            )
    }

    fn render_taskbar_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let hide_taskbar = self.settings.hide_taskbar_with_icons;

        let mut section = div().flex().flex_col().gap_4();

        section = section.child(Self::render_toggle(
            "taskbar-toggle",
            hide_taskbar,
            "Double-click hides taskbar",
            |this, _, _, _| {
                this.settings.hide_taskbar_with_icons = !this.settings.hide_taskbar_with_icons;
                this.apply_settings();
            },
            cx,
        ));

        // Taskbar toggle hotkey input
        let tb_hotkey_text = if self.capturing_taskbar_hotkey {
            "Press a key combination... (Esc to clear)".to_string()
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

        section = section.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_color(HideIconsTheme::text_secondary())
                        .text_sm()
                        .child("Taskbar toggle hotkey"),
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
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                            if !this.capturing_taskbar_hotkey {
                                return;
                            }
                            if event.keystroke.key.as_str() == "escape" {
                                this.settings.taskbar_hotkey = None;
                                this.capturing_taskbar_hotkey = false;
                                this.apply_settings();
                                cx.notify();
                                return;
                            }
                            if let Some(binding) =
                                SettingsView::parse_keystroke(&event.keystroke)
                            {
                                this.capturing_taskbar_hotkey = false;
                                // Validate: must differ from icon hotkey
                                if this.settings.hotkey.as_ref() == Some(&binding) {
                                    this.taskbar_hotkey_conflict = true;
                                    cx.notify();
                                    return;
                                }
                                this.settings.taskbar_hotkey = Some(binding);
                                this.apply_settings();
                                cx.notify();
                            }
                        })),
                )
                .children(tb_conflict),
        );

        section
    }

    fn render_system_tab(&mut self, cx: &mut Context<Self>) -> Div {
        let minimize_ix = match self.settings.minimize_behavior {
            MinimizeBehavior::Taskbar => 0,
            MinimizeBehavior::Tray => 1,
        };

        let close_ix = match self.settings.close_behavior {
            CloseBehavior::Exit => 0,
            CloseBehavior::Tray => 1,
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(Self::render_toggle(
                "startup-toggle",
                self.settings.start_with_windows,
                "Start with Windows",
                |this, _, _, _| {
                    this.settings.start_with_windows = !this.settings.start_with_windows;
                    this.apply_settings();
                },
                cx,
            ))
            .child(Self::render_toggle(
                "minimized-toggle",
                self.settings.start_minimized,
                "Start minimized to tray",
                |this, _, _, _| {
                    this.settings.start_minimized = !this.settings.start_minimized;
                    this.apply_settings();
                },
                cx,
            ))
            // Minimize behavior
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_color(HideIconsTheme::text_secondary())
                            .text_sm()
                            .child("When minimizing the window"),
                    )
                    .child(
                        RadioGroup::vertical("minimize-behavior")
                            .selected_index(Some(minimize_ix))
                            .on_click(cx.listener(|this, ix: &usize, _window, _cx| {
                                this.settings.minimize_behavior = match ix {
                                    0 => MinimizeBehavior::Taskbar,
                                    _ => MinimizeBehavior::Tray,
                                };
                                this.apply_settings();
                            }))
                            .child("Minimize to taskbar")
                            .child("Minimize to tray"),
                    ),
            )
            // Close behavior
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_color(HideIconsTheme::text_secondary())
                            .text_sm()
                            .child("When closing the window"),
                    )
                    .child(
                        RadioGroup::vertical("close-behavior")
                            .selected_index(Some(close_ix))
                            .on_click(cx.listener(|this, ix: &usize, _window, _cx| {
                                this.settings.close_behavior = match ix {
                                    0 => CloseBehavior::Exit,
                                    _ => CloseBehavior::Tray,
                                };
                                this.apply_settings();
                            }))
                            .child("Exit Polterdesk")
                            .child("Minimize to tray"),
                    ),
            )
    }
    fn render_about_tab(&mut self, _cx: &mut Context<Self>) -> Div {
        let logo_image = Arc::new(gpui::Image::from_bytes(
            gpui::ImageFormat::Png,
            include_bytes!("../../assets/logo.png").to_vec(),
        ));

        let mut content = div().flex().flex_col().gap_4();

        // Polterdesk section
        {
            let header = div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    img(ImageSource::from(logo_image))
                        .size(px(48.0))
                        .rounded_sm(),
                )
                .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child("Polterdesk"),
                    )
                    .child(
                        div()
                            .text_color(HideIconsTheme::text_secondary())
                            .text_sm()
                            .child("Toggle desktop icon visibility via hotkey or double-click."),
                    ),
            );

            let links = div()
                .flex()
                .gap_3()
                .text_sm()
                .child(
                    Link::new("website-link")
                        .href("https://polterdesk.com")
                        .child("Website"),
                )
                .child(
                    Link::new("github-link")
                        .href("https://github.com/EternityX/polterdesk")
                        .child("GitHub"),
                );

            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(header)
                    .child(links),
            );
        }

        // Separator
        content = content.child(
            div()
                .h(px(1.0))
                .bg(HideIconsTheme::border()),
        );

        // Licenses section
        content = content.child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_color(HideIconsTheme::text_secondary())
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Open Source Licenses"),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .text_sm()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child("GPUI — Zed Industries")
                                .child(
                                    Link::new("gpui-license")
                                        .href("https://github.com/zed-industries/zed/blob/main/LICENSE-APACHE")
                                        .text_xs()
                                        .child("Apache License 2.0"),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child("gpui-component — Longbridge")
                                .child(
                                    Link::new("gpui-component-license")
                                        .href("https://github.com/longbridge/gpui-component/blob/main/LICENSE-APACHE")
                                        .text_xs()
                                        .child("Apache License 2.0"),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_color(HideIconsTheme::text_secondary())
                        .text_xs()
                        .child("\u{00a9} 2026 EternityX \u{2014} MIT License"),
                ),
        );

        content
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_ix = TAB_ORDER
            .iter()
            .position(|t| *t == self.active_tab)
            .unwrap_or(0);

        let tab_content = match self.active_tab {
            SettingsTab::Icons => self.render_icons_tab(cx),
            SettingsTab::Taskbar => self.render_taskbar_tab(cx),
            SettingsTab::System => self.render_system_tab(cx),
            SettingsTab::About => self.render_about_tab(cx),
        };

        div()
            .size_full()
            .relative()
            .bg(HideIconsTheme::background())
            .text_color(HideIconsTheme::text_primary())
            .flex()
            .flex_col()
            // Tip alert
            .child({
                let tip = if self.settings.hide_taskbar_with_icons {
                    "Double-click on your desktop to toggle visibility of icons and taskbar."
                } else {
                    "Double-click on your desktop to toggle visibility of icons."
                };
                div()
                    .mx_4()
                    .mt_4()
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
            // Tab bar
            .child(
                div().px_4().pt_3().child(
                    TabBar::new("settings-tabs")
                        .w_full()
                        .segmented()
                        .selected_index(active_ix)
                        .on_click(cx.listener(|this, ix: &usize, _window, cx| {
                            if let Some(tab) = TAB_ORDER.get(*ix) {
                                this.active_tab = *tab;
                                cx.notify();
                            }
                        }))
                        .child(Tab::new().flex_1().label("Icons"))
                        .child(Tab::new().flex_1().label("Taskbar"))
                        .child(Tab::new().flex_1().label("System"))
                        .child(Tab::new().flex_1().label("About")),
                ),
            )
            // Scrollable content area
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .p_4()
                    .overflow_y_scrollbar()
                    .child(tab_content),
            )
            // Version number (absolute bottom-right)
            .child(
                div()
                    .absolute()
                    .bottom_2()
                    .right_4()
                    .text_color(HideIconsTheme::text_secondary())
                    .text_xs()
                    .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
            )
    }
}
