use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOWNA,
};

/// Hides the desktop icon listview.
pub fn hide_icons(listview: HWND) {
    // SAFETY: listview is a valid HWND obtained from find_desktop_listview().
    // SW_HIDE makes the window invisible without changing ownership or Z-order.
    unsafe {
        let _ = ShowWindow(listview, SW_HIDE);
    }
}

/// Shows the desktop icon listview without activating it.
pub fn show_icons(listview: HWND) {
    // SAFETY: listview is a valid HWND obtained from find_desktop_listview().
    // SW_SHOWNA displays the window without activating it, preserving focus.
    unsafe {
        let _ = ShowWindow(listview, SW_SHOWNA);
    }
}

/// Returns true if the desktop icon listview is currently visible.
#[allow(dead_code)]
pub fn is_visible(listview: HWND) -> bool {
    // SAFETY: listview is a valid HWND. IsWindowVisible is a read-only query.
    unsafe { IsWindowVisible(listview).as_bool() }
}
