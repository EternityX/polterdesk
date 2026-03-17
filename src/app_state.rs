use std::sync::{mpsc, Arc, Mutex};

use crate::settings::Settings;

/// Current visibility state of desktop icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleState {
    Visible,
    Hidden,
}

/// Messages sent between the WinAPI background thread and GPUI main thread.
#[derive(Debug)]
pub enum AppEvent {
    #[allow(dead_code)]
    ToggleTriggered,
    /// Separate taskbar hotkey pressed (only in separate mode).
    #[allow(dead_code)]
    TaskbarToggleTriggered,
    SettingsWindowRequested,
    ExitRequested,
    HotkeyConflict(crate::settings::HotkeyBinding),
    /// Taskbar hotkey registration failed - conflict detected.
    TaskbarHotkeyConflict(crate::settings::HotkeyBinding),
    NativeToggleDetected,
}

/// Per-icon data captured from the desktop listview.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IconPosition {
    pub index: usize,
    pub point: (i32, i32),
    pub bounds: (i32, i32, i32, i32),
}

/// Snapshot of all icon positions captured before hiding.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DesktopSnapshot {
    pub item_count: usize,
    pub positions: Vec<IconPosition>,
    pub captured_at: std::time::Instant,
}

/// Shared application state accessed by both threads.
pub struct AppState {
    pub toggle_state: ToggleState,
    /// Whether the taskbar is currently in app-controlled auto-hide mode.
    pub taskbar_hidden: bool,
    /// Original taskbar state from ABM_GETSTATE before we changed it.
    pub taskbar_original_state: Option<u32>,
    pub settings: Settings,
    pub snapshot: Option<DesktopSnapshot>,
    pub listview_hwnd: Option<isize>,
    pub gpui_tx: mpsc::Sender<AppEvent>,
    /// HWND of the WinAPI background thread's message-only window.
    /// Set by the WinAPI thread after creation; used by GPUI thread to post messages.
    pub winapi_hwnd: Option<isize>,
}

pub type SharedState = Arc<Mutex<AppState>>;

impl AppState {
    pub fn new(settings: Settings, tx: mpsc::Sender<AppEvent>) -> SharedState {
        Arc::new(Mutex::new(AppState {
            toggle_state: ToggleState::Visible,
            taskbar_hidden: false,
            taskbar_original_state: None,
            settings,
            snapshot: None,
            listview_hwnd: None,
            gpui_tx: tx,
            winapi_hwnd: None,
        }))
    }
}

/// Toggles the taskbar auto-hide state. Updates AppState and persists to settings.
pub fn perform_taskbar_toggle(state: &SharedState) {
    let mut guard = state.lock().unwrap();

    if guard.taskbar_hidden {
        restore_taskbar(&mut guard);
    } else {
        enable_taskbar_autohide(&mut guard);
    }
}

/// Enables taskbar auto-hide. Saves original state for exit restoration.
/// If already app-controlled, no-ops.
fn enable_taskbar_autohide(guard: &mut AppState) {
    if guard.taskbar_hidden {
        return; // Already under our control
    }

    match crate::desktop::taskbar::set_taskbar_autohide(true, None) {
        Ok(Some(original)) => {
            guard.taskbar_hidden = true;
            guard.taskbar_original_state = Some(original);
            guard.settings.taskbar_original_state = Some(original);
            let _ = guard.settings.save();
        }
        Ok(None) => {}
        Err(e) => eprintln!("Failed to set taskbar autohide: {e}"),
    }
}

/// Disables taskbar auto-hide during a toggle cycle.
/// Always forces auto-hide OFF regardless of what the original state was.
/// The saved original is kept for app-exit restoration.
fn restore_taskbar(guard: &mut AppState) {
    if !guard.taskbar_hidden {
        return;
    }
    // Force auto-hide off (pass Some(0) so the restore path sets ABS_ALWAYSONTOP only)
    match crate::desktop::taskbar::set_taskbar_autohide(false, Some(0)) {
        Ok(_) => {
            guard.taskbar_hidden = false;
            // Keep taskbar_original_state in settings for exit restoration;
            // clear it from runtime state since we're no longer controlling it.
            guard.taskbar_original_state = None;
            // Don't clear settings.taskbar_original_state here - it's needed
            // so that on exit we can restore the user's true original state.
        }
        Err(e) => eprintln!("Failed to restore taskbar: {e}"),
    }
}

/// Restores taskbar to the user's true original state. Used on app exit and crash recovery.
pub fn restore_taskbar_for_exit(guard: &mut AppState) {
    let original = guard
        .taskbar_original_state
        .or(guard.settings.taskbar_original_state);
    if guard.taskbar_hidden || original.is_some() {
        let _ = crate::desktop::taskbar::set_taskbar_autohide(false, original);
        guard.taskbar_hidden = false;
        guard.taskbar_original_state = None;
        guard.settings.taskbar_original_state = None;
        let _ = guard.settings.save();
    }
}

/// Performs the toggle operation: hide if visible, show if hidden.
/// Re-resolves the listview HWND if stale or not yet found.
pub fn perform_toggle(state: &SharedState) {
    use crate::desktop::{finder, positions, toggle};
    use windows::Win32::Foundation::HWND;

    let mut guard = state.lock().unwrap();

    // Resolve or re-resolve the listview handle
    let listview = match guard.listview_hwnd {
        Some(raw) => {
            let hwnd = HWND(raw as *mut _);
            // Verify the window still exists
            // SAFETY: IsWindow is a read-only query on a window handle.
            let valid =
                unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd)) }.as_bool();
            if valid {
                hwnd
            } else {
                match finder::find_desktop_listview() {
                    Some(h) => {
                        guard.listview_hwnd = Some(h.0 as isize);
                        h
                    }
                    None => {
                        eprintln!("Could not find desktop listview");
                        return;
                    }
                }
            }
        }
        None => match finder::find_desktop_listview() {
            Some(h) => {
                guard.listview_hwnd = Some(h.0 as isize);
                h
            }
            None => {
                eprintln!("Could not find desktop listview");
                return;
            }
        },
    };

    let combined_mode = guard.settings.hide_taskbar_with_icons;

    match guard.toggle_state {
        ToggleState::Visible => {
            // Save snapshot before hiding
            if let Some(snapshot) = positions::save_snapshot(listview) {
                guard.snapshot = Some(snapshot);
            }
            toggle::hide_icons(listview);
            guard.toggle_state = ToggleState::Hidden;

            // In combined mode, also enable taskbar auto-hide
            if combined_mode {
                enable_taskbar_autohide(&mut guard);
            }
        }
        ToggleState::Hidden => {
            toggle::show_icons(listview);
            if let Some(ref snapshot) = guard.snapshot {
                positions::restore_snapshot(listview, snapshot);
            }
            guard.snapshot = None;
            guard.toggle_state = ToggleState::Visible;

            // In combined mode, restore taskbar
            if combined_mode {
                restore_taskbar(&mut guard);
            }
        }
    }
}
