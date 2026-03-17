use crate::app_state::SharedState;
use std::cell::UnsafeCell;
use std::sync::OnceLock;
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetClassNameW, GetSystemMetrics, SetWindowsHookExW, UnhookWindowsHookEx,
    WindowFromPoint, HHOOK, MSLLHOOKSTRUCT, SM_CXDOUBLECLK, SM_CYDOUBLECLK, WH_MOUSE_LL,
    WM_LBUTTONDOWN,
};

static HOOK_STATE: OnceLock<SharedState> = OnceLock::new();

struct ClickTracker {
    last_point: POINT,
    last_time: std::time::Instant,
    has_previous: bool,
}

struct SyncClickTracker(UnsafeCell<ClickTracker>);

// SAFETY: CLICK_TRACKER is only accessed from the hook thread (single-threaded access
// via the WH_MOUSE_LL callback, which always runs on the thread that installed the hook).
unsafe impl Sync for SyncClickTracker {}

static CLICK_TRACKER: SyncClickTracker = SyncClickTracker(UnsafeCell::new(ClickTracker {
    last_point: POINT { x: 0, y: 0 },
    // SAFETY: Instant is repr(transparent) over a platform Duration; zero-init is safe
    // because has_previous starts false, so last_time is never read before being written.
    last_time: unsafe { std::mem::MaybeUninit::zeroed().assume_init() },
    has_previous: false,
}));

/// Returns true if the current click forms a double-click with the previous click.
fn is_double_click(pt: POINT) -> bool {
    // SAFETY: GetDoubleClickTime and GetSystemMetrics are read-only system queries.
    let dblclick_time = unsafe { GetDoubleClickTime() };
    let cx = unsafe { GetSystemMetrics(SM_CXDOUBLECLK) };
    let cy = unsafe { GetSystemMetrics(SM_CYDOUBLECLK) };

    // SAFETY: CLICK_TRACKER is only accessed from the hook thread (single-threaded access).
    let tracker = unsafe { &mut *CLICK_TRACKER.0.get() };

    if !tracker.has_previous {
        tracker.last_point = pt;
        tracker.last_time = std::time::Instant::now();
        tracker.has_previous = true;
        return false;
    }

    let elapsed = tracker.last_time.elapsed().as_millis() as u32;
    let dx = (pt.x - tracker.last_point.x).abs();
    let dy = (pt.y - tracker.last_point.y).abs();

    let is_dblclick = elapsed < dblclick_time && dx <= cx && dy <= cy;

    tracker.last_point = pt;
    tracker.last_time = std::time::Instant::now();
    tracker.has_previous = !is_dblclick; // Reset after double-click

    is_dblclick
}

/// Returns true if the point is over a desktop window.
/// Checks for Progman, WorkerW, SysListView32, and SHELLDLL_DefView (the parent
/// container of SysListView32 that receives hits when the listview is hidden).
fn is_click_on_desktop(pt: POINT) -> bool {
    // SAFETY: WindowFromPoint is a read-only query.
    let hwnd = unsafe { WindowFromPoint(pt) };
    if hwnd.is_invalid() {
        return false;
    }

    let mut class_name = [0u16; 256];
    // SAFETY: hwnd is a valid window handle from WindowFromPoint.
    let len = unsafe { GetClassNameW(hwnd, &mut class_name) } as usize;
    if len == 0 {
        return false;
    }

    let name = String::from_utf16_lossy(&class_name[..len]);
    matches!(
        name.as_str(),
        "Progman" | "WorkerW" | "SysListView32" | "SHELLDLL_DefView"
    )
}

/// Installs the WH_MOUSE_LL hook. Must be called from the thread that pumps messages.
pub fn install(state: SharedState) -> Option<HHOOK> {
    let _ = HOOK_STATE.set(state);

    // SAFETY: SetWindowsHookExW installs a low-level mouse hook. The callback
    // runs on this thread's message pump. hmod=None and thread_id=0 means global.
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0) };
    match hook {
        Ok(h) => Some(h),
        Err(e) => {
            eprintln!("Failed to install mouse hook: {e}");
            None
        }
    }
}

/// Uninstalls the WH_MOUSE_LL hook.
pub fn uninstall(hook: HHOOK) {
    // SAFETY: UnhookWindowsHookEx removes a previously installed hook.
    let _ = unsafe { UnhookWindowsHookEx(hook) };
}

/// Low-level mouse hook callback.
unsafe extern "system" fn mouse_hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode < 0 {
        // SAFETY: Negative ncode means we must pass to next hook.
        return unsafe { CallNextHookEx(None, ncode, wparam, lparam) };
    }

    let Some(state) = HOOK_STATE.get() else {
        return unsafe { CallNextHookEx(None, ncode, wparam, lparam) };
    };

    // SAFETY: lparam points to a valid MSLLHOOKSTRUCT provided by the system.
    let mouse_info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
    let pt = mouse_info.pt;
    let msg = wparam.0 as u32;

    // Only handle left button down on the desktop
    if msg == WM_LBUTTONDOWN && is_click_on_desktop(pt) && is_double_click(pt) {
        // Check if the click hit an actual icon - if so, let the system handle it
        // (e.g. open the program). Only toggle when clicking empty desktop space.
        let hit_icon = {
            let guard = state.lock().unwrap();
            if guard.toggle_state == crate::app_state::ToggleState::Visible {
                if let Some(raw_hwnd) = guard.listview_hwnd {
                    let listview = windows::Win32::Foundation::HWND(raw_hwnd as *mut _);
                    crate::desktop::positions::hit_test_icon(listview, (pt.x, pt.y))
                } else {
                    false
                }
            } else {
                false
            }
        };

        if !hit_icon {
            // perform_toggle handles taskbar too when hide_taskbar_with_icons is on
            crate::app_state::perform_toggle(state);
            return LRESULT(1); // Swallow the double-click
        }
    }

    // SAFETY: Pass unhandled messages to the next hook.
    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}
