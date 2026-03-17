use crate::settings::Settings;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::UI::Shell::{SHAppBarMessage, APPBARDATA};
use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

const ABM_GETSTATE: u32 = 4;
const ABM_SETSTATE: u32 = 10;
const ABS_AUTOHIDE: u32 = 0x01;
const ABS_ALWAYSONTOP: u32 = 0x02;

/// Returns the current taskbar auto-hide bitmask via ABM_GETSTATE.
pub fn get_taskbar_state() -> u32 {
    let mut abd = make_appbardata();
    // SAFETY: SHAppBarMessage with ABM_GETSTATE is a read-only query on the taskbar state.
    // The APPBARDATA struct is fully initialized with cbSize and a valid taskbar hWnd.
    let result = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut abd) };
    result as u32
}

/// Toggles the taskbar auto-hide state.
///
/// When `enable` is true: captures the current state via ABM_GETSTATE, then enables
/// auto-hide via ABM_SETSTATE. Returns `Some(original_state)`.
///
/// When `enable` is false: restores from `original_state`. If the original state was 0
/// (no flags), sets ABS_ALWAYSONTOP for correct behavior on Windows 7+. Returns `None`.
pub fn set_taskbar_autohide(
    enable: bool,
    original_state: Option<u32>,
) -> windows::core::Result<Option<u32>> {
    let mut abd = make_appbardata();

    if enable {
        // Capture original state before modifying
        // SAFETY: ABM_GETSTATE is a read-only query; APPBARDATA is fully initialized.
        let original = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut abd) } as u32;

        // Enable auto-hide
        abd.lParam = LPARAM((ABS_AUTOHIDE | ABS_ALWAYSONTOP) as isize);
        // SAFETY: ABM_SETSTATE modifies the taskbar auto-hide setting system-wide.
        // The APPBARDATA struct has a valid taskbar hWnd and lParam set to the desired flags.
        unsafe { SHAppBarMessage(ABM_SETSTATE, &mut abd) };

        Ok(Some(original))
    } else {
        // Restore original state
        let restore_flags = match original_state {
            Some(0) => ABS_ALWAYSONTOP, // Win7+ always-on-top; don't restore to bare 0
            Some(flags) => flags | ABS_ALWAYSONTOP,
            None => ABS_ALWAYSONTOP,
        };
        abd.lParam = LPARAM(restore_flags as isize);
        // SAFETY: ABM_SETSTATE restores the taskbar to its original state.
        // The APPBARDATA struct has a valid taskbar hWnd and lParam set to the restore flags.
        unsafe { SHAppBarMessage(ABM_SETSTATE, &mut abd) };

        Ok(None)
    }
}

/// Checks if the taskbar was left in app-controlled auto-hide from a previous crash.
/// If `settings.taskbar_original_state` is `Some`, restores the taskbar and clears the field.
pub fn restore_taskbar_if_needed(settings: &mut Settings) -> windows::core::Result<bool> {
    if let Some(original) = settings.taskbar_original_state.take() {
        let mut abd = make_appbardata();
        let restore_flags = if original == 0 {
            ABS_ALWAYSONTOP
        } else {
            original | ABS_ALWAYSONTOP
        };
        abd.lParam = LPARAM(restore_flags as isize);
        // SAFETY: ABM_SETSTATE restores the taskbar to its pre-crash state.
        // The APPBARDATA struct has a valid taskbar hWnd and lParam set to the restore flags.
        unsafe { SHAppBarMessage(ABM_SETSTATE, &mut abd) };
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Constructs an APPBARDATA struct with the taskbar window handle.
fn make_appbardata() -> APPBARDATA {
    let class_name = windows::core::HSTRING::from("Shell_TrayWnd");
    // SAFETY: FindWindowW with "Shell_TrayWnd" locates the primary taskbar window.
    // This is a read-only operation that always succeeds on a running Windows desktop.
    let taskbar_hwnd = unsafe { FindWindowW(&class_name, None) }.unwrap_or(HWND::default());

    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: taskbar_hwnd,
        uCallbackMessage: 0,
        uEdge: 0,
        rc: RECT::default(),
        lParam: LPARAM(0),
    }
}
