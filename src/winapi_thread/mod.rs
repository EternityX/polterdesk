pub mod hook;
pub mod hotkey;
pub mod tray;

use crate::app_state::{AppEvent, SharedState, ToggleState};
use std::thread;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
    PostQuitMessage, RegisterClassW, SetTimer, SetWindowLongPtrW, TranslateMessage, GWLP_USERDATA,
    HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_HOTKEY, WM_TIMER, WNDCLASSW,
};

const TIMER_NATIVE_CHECK: usize = 100;
const NATIVE_CHECK_INTERVAL_MS: u32 = 250;
/// Custom message posted by the GPUI thread to trigger hotkey re-registration.
pub const WM_REREGISTER_HOTKEY: u32 = 0x0402; // WM_APP + 2

/// Spawns the WinAPI background thread with its own message pump.
pub fn spawn(state: SharedState) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        run_message_loop(state);
    })
}

fn run_message_loop(state: SharedState) {
    unsafe {
        let class_name = windows::core::HSTRING::from("PolterdeskMsgWnd");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        // SAFETY: Registering a window class with valid function pointer and class name.
        RegisterClassW(&wc);

        // SAFETY: Creating a message-only window with HWND_MESSAGE as parent.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            &class_name,
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        );
        if hwnd.is_err() {
            eprintln!("Failed to create message-only window");
            return;
        }
        let hwnd = hwnd.unwrap();

        // Store state pointer in window user data so wnd_proc can access it.
        // Box::into_raw leaks the clone; it's freed after the message loop exits.
        let state_ptr = Box::into_raw(Box::new(state.clone()));
        // SAFETY: Setting user data on our own valid window.
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

        // Store the message-only window HWND so the GPUI thread can post messages
        {
            let mut guard = state.lock().unwrap();
            guard.winapi_hwnd = Some(hwnd.0 as isize);
        }

        // Register hotkeys
        {
            let guard = state.lock().unwrap();
            if let Some(ref binding) = guard.settings.hotkey {
                if let Err(e) = hotkey::register(hwnd, binding) {
                    eprintln!("Failed to register hotkey: {:?}", e);
                    let _ = guard
                        .gpui_tx
                        .send(AppEvent::HotkeyConflict(binding.clone()));
                }
            }

            // Register taskbar hotkey only in separate mode
            if !guard.settings.hide_taskbar_with_icons {
                if let Some(ref tb_binding) = guard.settings.taskbar_hotkey {
                    if let Err(e) = hotkey::register_taskbar(hwnd, tb_binding) {
                        eprintln!("Failed to register taskbar hotkey: {:?}", e);
                        let _ = guard
                            .gpui_tx
                            .send(AppEvent::TaskbarHotkeyConflict(tb_binding.clone()));
                    }
                }
            }
        }

        // Load the application icon from the embedded resource for the tray
        let hicon = {
            use windows::Win32::System::LibraryLoader::GetModuleHandleW;
            use windows::Win32::UI::WindowsAndMessaging::{
                LoadImageW, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED,
            };

            // SAFETY: GetModuleHandleW(None) returns the HINSTANCE of the current exe.
            let hinstance = GetModuleHandleW(None).ok();
            let icon = hinstance.and_then(|h| {
                // SAFETY: LoadImageW loads the icon embedded by winres (resource ID 1).
                // LR_SHARED avoids needing to manually destroy the icon.
                LoadImageW(
                    Some(h.into()),
                    // MAKEINTRESOURCE(1) - resource ID as a pointer
                    #[allow(clippy::manual_dangling_ptr)]
                    windows::core::PCWSTR::from_raw(1 as *const u16),
                    IMAGE_ICON,
                    0,
                    0,
                    LR_DEFAULTSIZE | LR_SHARED,
                )
                .ok()
            });
            match icon {
                Some(h) => windows::Win32::UI::WindowsAndMessaging::HICON(h.0),
                None => {
                    // Fallback to system application icon
                    windows::Win32::UI::WindowsAndMessaging::LoadIconW(
                        None,
                        windows::Win32::UI::WindowsAndMessaging::IDI_APPLICATION,
                    )
                    .unwrap_or_default()
                }
            }
        };
        tray::add(hwnd, hicon);

        // Install the low-level mouse hook for desktop double-click detection
        let mouse_hook = hook::install(state.clone());

        // Start a 250ms timer to detect native "Show Desktop Icons" toggle
        // SAFETY: SetTimer with a message-only window; WM_TIMER fires in the message loop.
        SetTimer(
            Some(hwnd),
            TIMER_NATIVE_CHECK,
            NATIVE_CHECK_INTERVAL_MS,
            None,
        );

        // Message pump - must run continuously to keep WH_MOUSE_LL hook alive
        let mut msg = MSG::default();
        // SAFETY: Standard Win32 message loop. GetMessageW blocks until a message arrives.
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            match msg.message {
                WM_HOTKEY => {
                    let hotkey_id = msg.wParam.0 as i32;
                    if hotkey_id == hotkey::TASKBAR_HOTKEY_ID {
                        crate::app_state::perform_taskbar_toggle(&state);
                    } else {
                        crate::app_state::perform_toggle(&state);
                    }
                    update_tray_tooltip(hwnd, &state);
                }
                WM_TIMER if msg.wParam.0 == TIMER_NATIVE_CHECK => {
                    check_native_toggle(&state);
                }
                _ => {}
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Cleanup
        if let Some(h) = mouse_hook {
            hook::uninstall(h);
        }
        hotkey::unregister(hwnd);
        hotkey::unregister_taskbar(hwnd);
        tray::remove(hwnd);

        // Free the state pointer we stored in user data
        let _ = Box::from_raw(state_ptr);
    }
}

/// Checks if the desktop listview became visible externally while we think it's hidden.
/// Also checks if the taskbar auto-hide state was changed externally.
fn check_native_toggle(state: &SharedState) {
    let mut guard = state.lock().unwrap();

    // Check desktop icon native toggle
    if guard.toggle_state == ToggleState::Hidden {
        if let Some(raw_hwnd) = guard.listview_hwnd {
            let hwnd = HWND(raw_hwnd as *mut _);
            // SAFETY: IsWindowVisible is a read-only query.
            let visible =
                unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd) }.as_bool();
            if visible {
                // Native toggle detected - update state
                guard.toggle_state = ToggleState::Visible;
                guard.snapshot = None;
                let _ = guard.gpui_tx.send(AppEvent::NativeToggleDetected);
            }
        }
    }

    // Check taskbar external state change
    if guard.taskbar_hidden {
        let current_state = crate::desktop::taskbar::get_taskbar_state();
        // We expect ABS_AUTOHIDE to be set (bit 0). If it's gone, user changed it externally.
        let autohide_active = (current_state & 0x01) != 0;
        if !autohide_active {
            // External change detected - sync internal state
            guard.taskbar_hidden = false;
            guard.taskbar_original_state = None;
            guard.settings.taskbar_original_state = None;
            let _ = guard.settings.save();
        }
    }
}

fn update_tray_tooltip(hwnd: HWND, state: &SharedState) {
    let guard = state.lock().unwrap();
    let icons_visible = guard.toggle_state == ToggleState::Visible;
    let taskbar_hidden = guard.taskbar_hidden;
    drop(guard);
    tray::update_tooltip(hwnd, icons_visible, taskbar_hidden);
}

fn handle_tray_message(lparam: LPARAM, hwnd: HWND, state: &SharedState) {
    let event = (lparam.0 & 0xFFFF) as u32;
    match event {
        // WM_RBUTTONUP - show context menu
        0x0205 => {
            let guard = state.lock().unwrap();
            let toggle_state = guard.toggle_state;
            let start_with_windows = guard.settings.start_with_windows;
            let show_taskbar_toggle = !guard.settings.hide_taskbar_with_icons;
            let taskbar_hidden = guard.taskbar_hidden;
            drop(guard);
            tray::show_context_menu(
                hwnd,
                toggle_state,
                start_with_windows,
                show_taskbar_toggle,
                taskbar_hidden,
            );
        }
        // WM_LBUTTONUP - single click opens settings window
        0x0202 => {
            let guard = state.lock().unwrap();
            let _ = guard.gpui_tx.send(AppEvent::SettingsWindowRequested);
        }
        // WM_LBUTTONDBLCLK - double click also opens settings window
        0x0203 => {
            let guard = state.lock().unwrap();
            let _ = guard.gpui_tx.send(AppEvent::SettingsWindowRequested);
        }
        _ => {}
    }
}

fn handle_command(wparam: WPARAM, state: &SharedState, hwnd: HWND) {
    let id = (wparam.0 & 0xFFFF) as u32;
    match id {
        tray::IDM_OPEN => {
            let guard = state.lock().unwrap();
            let _ = guard.gpui_tx.send(AppEvent::SettingsWindowRequested);
        }
        tray::IDM_TOGGLE => {
            crate::app_state::perform_toggle(state);
            update_tray_tooltip(hwnd, state);
        }
        tray::IDM_TOGGLE_TASKBAR => {
            crate::app_state::perform_taskbar_toggle(state);
            update_tray_tooltip(hwnd, state);
        }
        tray::IDM_STARTUP => {
            let mut guard = state.lock().unwrap();
            let new_val = !guard.settings.start_with_windows;
            guard.settings.start_with_windows = new_val;
            let _ = guard.settings.save();
            drop(guard);
            crate::startup::set_startup(new_val);
        }
        tray::IDM_CLOSE => {
            perform_exit(state, hwnd);
        }
        _ => {}
    }
}

/// Sends ExitRequested to the GPUI thread. The GPUI handler does all restoration.
/// PostQuitMessage breaks the WinAPI message loop after the GPUI thread signals quit.
pub fn perform_exit(state: &SharedState, _hwnd: HWND) {
    let guard = state.lock().unwrap();
    let _ = guard.gpui_tx.send(AppEvent::ExitRequested);
    drop(guard);
    // SAFETY: PostQuitMessage sends WM_QUIT to break the message loop.
    unsafe { PostQuitMessage(0) };
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Retrieve state pointer from window user data
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const SharedState;
    if !state_ptr.is_null() {
        // SAFETY: state_ptr is valid for the lifetime of the window (set in run_message_loop).
        let state = unsafe { &*state_ptr };

        match msg {
            WM_COMMAND => {
                handle_command(wparam, state, hwnd);
                return LRESULT(0);
            }
            m if m == tray::WM_TRAYICON => {
                handle_tray_message(lparam, hwnd, state);
                return LRESULT(0);
            }
            m if m == WM_REREGISTER_HOTKEY => {
                let guard = state.lock().unwrap();
                let binding = guard.settings.hotkey.clone();
                let hide_taskbar_with_icons = guard.settings.hide_taskbar_with_icons;
                let taskbar_hotkey = guard.settings.taskbar_hotkey.clone();
                drop(guard);

                // Re-register icon hotkey (or unregister if cleared)
                if let Some(ref b) = binding {
                    if let Err(e) = hotkey::reregister(hwnd, b) {
                        eprintln!("Failed to re-register hotkey: {:?}", e);
                        let guard = state.lock().unwrap();
                        let _ = guard.gpui_tx.send(AppEvent::HotkeyConflict(b.clone()));
                    }
                } else {
                    hotkey::unregister(hwnd);
                }

                // Re-register taskbar hotkey (or unregister if in combined mode)
                if !hide_taskbar_with_icons {
                    if let Some(ref tb_binding) = taskbar_hotkey {
                        if let Err(e) = hotkey::reregister_taskbar(hwnd, tb_binding) {
                            eprintln!("Failed to re-register taskbar hotkey: {:?}", e);
                            let guard = state.lock().unwrap();
                            let _ = guard
                                .gpui_tx
                                .send(AppEvent::TaskbarHotkeyConflict(tb_binding.clone()));
                        }
                    } else {
                        hotkey::unregister_taskbar(hwnd);
                    }
                } else {
                    hotkey::unregister_taskbar(hwnd);
                }

                return LRESULT(0);
            }
            _ => {}
        }
    }

    // SAFETY: DefWindowProcW handles all messages we don't process.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
