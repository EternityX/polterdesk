pub mod hotkey_input;
pub mod settings_window;
pub mod theme;

use crate::app_state::AppEvent;
use crate::settings::Settings;
use gpui::*;
use settings_window::SettingsView;
use std::sync::mpsc;

/// Helper to extract the HWND from a GPUI Window and call a closure with it.
fn with_hwnd(window: &Window, f: impl FnOnce(windows::Win32::Foundation::HWND)) {
    if let Ok(rwh) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::Win32(handle) = rwh.as_raw() {
            f(windows::Win32::Foundation::HWND(handle.hwnd.get() as *mut _));
        }
    }
}

/// Opens the settings window.
/// - Minimize button hides to tray.
/// - Close button shows a confirmation dialog.
///
/// Returns the window handle for later re-show.
pub fn open_settings(
    cx: &mut App,
    settings: Settings,
    event_tx: mpsc::Sender<AppEvent>,
    state: crate::app_state::SharedState,
) -> Option<AnyWindowHandle> {
    let bounds = Bounds::centered(None, size(px(450.0), px(330.0)), cx);
    let window_handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("Polterdesk v{}", env!("CARGO_PKG_VERSION")).into()),
                    ..Default::default()
                }),
                is_resizable: false,
                ..Default::default()
            },
            |window, cx| {
                // Remove maximize button
                with_hwnd(window, |hwnd| {
                    use windows::Win32::UI::WindowsAndMessaging::{
                        GetWindowLongW, SetWindowLongW, GWL_STYLE, WS_MAXIMIZEBOX,
                    };
                    // SAFETY: Getting and setting window style on our own valid HWND.
                    unsafe {
                        let style = GetWindowLongW(hwnd, GWL_STYLE);
                        SetWindowLongW(hwnd, GWL_STYLE, style & !(WS_MAXIMIZEBOX.0 as i32));
                    }
                });

                // Intercept close: hide to tray instead of closing
                window.on_window_should_close(cx, move |window, _cx| {
                    with_hwnd(window, |hwnd| {
                        // SAFETY: Hiding the window to tray on close button click.
                        unsafe {
                            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                                hwnd,
                                windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
                            );
                        }
                    });
                    false // Prevent GPUI from destroying the window
                });

                let view = cx.new(|cx| SettingsView::new(settings, event_tx, state, cx));
                cx.new(|cx| gpui_component::Root::new(view.clone(), window, cx))
            },
        )
        .ok();

    window_handle.map(|h| h.into())
}

/// Checks if the window is minimized and hides it instead (minimize to tray).
/// Called from the polling loop in main.rs.
pub fn check_minimize_to_tray(handle: AnyWindowHandle, cx: &mut App) {
    handle
        .update(cx, |_, window, _cx| {
            with_hwnd(window, |hwnd| {
                // SAFETY: IsIconic is a read-only query.
                let is_minimized =
                    unsafe { windows::Win32::UI::WindowsAndMessaging::IsIconic(hwnd) }.as_bool();
                if is_minimized {
                    // SAFETY: SW_HIDE hides the window; SW_RESTORE is needed first
                    // to clear the minimized state so re-show works correctly later.
                    unsafe {
                        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                            hwnd,
                            windows::Win32::UI::WindowsAndMessaging::SW_RESTORE,
                        );
                        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                            hwnd,
                            windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
                        );
                    }
                }
            });
        })
        .ok();
}

/// Re-shows a previously hidden settings window.
pub fn show_window(handle: AnyWindowHandle, cx: &mut App) {
    handle
        .update(cx, |_, window, _cx| {
            with_hwnd(window, |hwnd| {
                // SAFETY: Re-showing a previously hidden window and bringing to foreground.
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                        hwnd,
                        windows::Win32::UI::WindowsAndMessaging::SW_SHOW,
                    );
                    let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
                }
            });
        })
        .ok();
}
