use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, FindWindowExW, FindWindowW, GetClassNameW, SendMessageW,
};
use windows::core::PCWSTR;

/// Locates the SysListView32 that hosts desktop icons.
///
/// Strategy:
/// 1. Find Progman, send 0x052C to ensure WorkerW exists.
/// 2. Enumerate WorkerW siblings looking for one with a SysListView32 child.
/// 3. Windows 11 fallback: check for SysListView32 directly under Progman.
pub fn find_desktop_listview() -> Option<HWND> {
    unsafe {
        // SAFETY: FindWindowW with known class names; no aliasing or lifetime concerns.
        let progman = FindWindowW(
            &windows::core::HSTRING::from("Progman"),
            PCWSTR::null(),
        ).ok()?;

        // SAFETY: Progman is a well-known shell window. This undocumented message
        // ensures the WorkerW hierarchy is spawned (used by animated wallpapers).
        SendMessageW(progman, 0x052C, None, None);

        // Try to find SysListView32 under a WorkerW sibling
        let mut worker_w = FindWindowExW(
            None,
            None,
            &windows::core::HSTRING::from("WorkerW"),
            PCWSTR::null(),
        ).ok();

        while let Some(ww) = worker_w {
            if let Ok(listview) = FindWindowExW(
                Some(ww),
                None,
                &windows::core::HSTRING::from("SysListView32"),
                PCWSTR::null(),
            ) {
                return Some(listview);
            }

            worker_w = FindWindowExW(
                None,
                Some(ww),
                &windows::core::HSTRING::from("WorkerW"),
                PCWSTR::null(),
            ).ok();
        }

        // Windows 11 fallback: SysListView32 may be a direct child of Progman
        if let Ok(fallback) = FindWindowExW(
            Some(progman),
            None,
            &windows::core::HSTRING::from("SysListView32"),
            PCWSTR::null(),
        ) {
            return Some(fallback);
        }

        // Final fallback: enumerate all children of Progman
        let mut result: Option<HWND> = None;
        let result_ptr = &mut result as *mut Option<HWND>;
        // SAFETY: We pass a valid pointer to our local `result` variable.
        // The callback only writes to it through the LPARAM.
        let _ = EnumChildWindows(
            Some(progman),
            Some(enum_child_proc),
            LPARAM(result_ptr as isize),
        );

        result
    }
}

/// Callback for EnumChildWindows that looks for SysListView32.
unsafe extern "system" fn enum_child_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let mut class_name = [0u16; 256];
    // SAFETY: hwnd is a valid window handle provided by the system enumeration.
    let len = unsafe { GetClassNameW(hwnd, &mut class_name) } as usize;
    if len > 0 {
        let name = String::from_utf16_lossy(&class_name[..len]);
        if name == "SysListView32" {
            // SAFETY: lparam contains a pointer to Option<HWND> that we control.
            let result = unsafe { &mut *(lparam.0 as *mut Option<HWND>) };
            *result = Some(hwnd);
            return windows::core::BOOL(0); // Stop enumeration
        }
    }
    windows::core::BOOL(1) // Continue enumeration
}
