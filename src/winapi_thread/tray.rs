use crate::app_state::ToggleState;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, SetForegroundWindow, TrackPopupMenu,
    HICON, MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
};

pub const IDM_OPEN: u32 = 1000;
pub const IDM_TOGGLE: u32 = 1001;
pub const IDM_STARTUP: u32 = 1003;
pub const IDM_CLOSE: u32 = 1004;
pub const IDM_TOGGLE_TASKBAR: u32 = 1005;

const TRAY_ICON_ID: u32 = 1;
pub const WM_TRAYICON: u32 = 0x0401; // WM_APP + 1

fn make_nid(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        ..Default::default()
    }
}

fn set_tooltip(nid: &mut NOTIFYICONDATAW, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let len = wide.len().min(nid.szTip.len());
    nid.szTip[..len].copy_from_slice(&wide[..len]);
}

/// Adds the application icon to the system tray.
pub fn add(hwnd: HWND, hicon: HICON) {
    let mut nid = make_nid(hwnd);
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = hicon;
    set_tooltip(&mut nid, "Polterdesk \u{2014} Icons visible");

    // SAFETY: nid is fully initialized with valid hwnd and icon.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

/// Removes the tray icon.
pub fn remove(hwnd: HWND) {
    let nid = make_nid(hwnd);
    // SAFETY: nid identifies the icon to remove.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

/// Updates the tray tooltip to reflect current icon and taskbar state.
pub fn update_tooltip(hwnd: HWND, icons_visible: bool, taskbar_hidden: bool) {
    let mut nid = make_nid(hwnd);
    nid.uFlags = NIF_TIP;
    let icons_str = if icons_visible { "Visible" } else { "Hidden" };
    let taskbar_str = if taskbar_hidden {
        "Auto-hide"
    } else {
        "Normal"
    };
    let text = format!("Polterdesk \u{2014} Icons: {icons_str} | Taskbar: {taskbar_str}");
    set_tooltip(&mut nid, &text);

    // SAFETY: Modifying the tooltip of an existing tray icon.
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

/// Shows the right-click context menu.
pub fn show_context_menu(
    hwnd: HWND,
    toggle_state: ToggleState,
    start_with_windows: bool,
    show_taskbar_toggle: bool,
    taskbar_hidden: bool,
) {
    unsafe {
        // SAFETY: CreatePopupMenu creates a new empty menu.
        let menu = CreatePopupMenu();
        let Ok(menu) = menu else { return };

        let open_label: Vec<u16> = "Open\0".encode_utf16().collect();
        let toggle_label: Vec<u16> = match toggle_state {
            ToggleState::Visible => "Hide Icons\0".encode_utf16().collect(),
            ToggleState::Hidden => "Show Icons\0".encode_utf16().collect(),
        };
        let startup_label: Vec<u16> = "Start with Windows\0".encode_utf16().collect();
        let close_label: Vec<u16> = "Close\0".encode_utf16().collect();

        // SAFETY: AppendMenuW adds items to the valid menu handle.
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            IDM_OPEN as usize,
            windows::core::PCWSTR(open_label.as_ptr()),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            IDM_TOGGLE as usize,
            windows::core::PCWSTR(toggle_label.as_ptr()),
        );

        // Show "Toggle Taskbar" only in separate mode
        if show_taskbar_toggle {
            let taskbar_label: Vec<u16> = if taskbar_hidden {
                "Show Taskbar\0".encode_utf16().collect()
            } else {
                "Hide Taskbar\0".encode_utf16().collect()
            };
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                IDM_TOGGLE_TASKBAR as usize,
                windows::core::PCWSTR(taskbar_label.as_ptr()),
            );
        }

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());

        let startup_flags = if start_with_windows {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING | MF_UNCHECKED
        };
        let _ = AppendMenuW(
            menu,
            startup_flags,
            IDM_STARTUP as usize,
            windows::core::PCWSTR(startup_label.as_ptr()),
        );

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            IDM_CLOSE as usize,
            windows::core::PCWSTR(close_label.as_ptr()),
        );

        // Get cursor position for menu placement
        let mut pt = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut pt);

        // SAFETY: SetForegroundWindow required before TrackPopupMenu to dismiss properly.
        let _ = SetForegroundWindow(hwnd);
        // SAFETY: TrackPopupMenu shows the menu at the cursor position.
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        // SAFETY: Destroying the menu after it's been dismissed.
        let _ = DestroyMenu(menu);
    }
}
