use crate::app_state::{DesktopSnapshot, IconPosition};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Memory::{
    MEM_COMMIT, MEM_RELEASE, PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx,
};
use windows::Win32::System::Threading::{
    PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, OpenProcess,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowThreadProcessId, SendMessageW,
};

const LVM_FIRST: u32 = 0x1000;
const LVM_GETITEMCOUNT: u32 = LVM_FIRST + 4;
const LVM_GETITEMPOSITION: u32 = LVM_FIRST + 16;
const LVM_SETITEMPOSITION: u32 = LVM_FIRST + 15;
const LVM_GETITEMRECT: u32 = LVM_FIRST + 14;
const LVM_HITTEST: u32 = LVM_FIRST + 18;
const LVIR_BOUNDS: i32 = 0;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LvHitTestInfo {
    pt: Point,
    flags: u32,
    item: i32,
    sub_item: i32,
    group: i32,
}

/// Captures a snapshot of all desktop icon positions and bounding rects.
pub fn save_snapshot(listview: HWND) -> Option<DesktopSnapshot> {
    unsafe {
        // SAFETY: listview is valid; LVM_GETITEMCOUNT requires no special memory.
        let count = SendMessageW(listview, LVM_GETITEMCOUNT, None, None).0 as usize;

        let mut pid: u32 = 0;
        // SAFETY: listview is valid; pid is a valid pointer to local stack memory.
        GetWindowThreadProcessId(listview, Some(&mut pid));
        if pid == 0 {
            return None;
        }

        // SAFETY: Opening explorer.exe process with VM operation permissions.
        let hproc = OpenProcess(
            PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE,
            false,
            pid,
        )
        .ok()?;

        // SAFETY: Allocating memory in the remote process for cross-process reads.
        let remote_pt = VirtualAllocEx(
            hproc,
            None,
            std::mem::size_of::<Point>(),
            MEM_COMMIT,
            PAGE_READWRITE,
        );
        if remote_pt.is_null() {
            let _ = windows::Win32::Foundation::CloseHandle(hproc);
            return None;
        }

        // SAFETY: Allocating memory in the remote process for RECT reads.
        let remote_rect = VirtualAllocEx(
            hproc,
            None,
            std::mem::size_of::<Rect>(),
            MEM_COMMIT,
            PAGE_READWRITE,
        );
        if remote_rect.is_null() {
            let _ = VirtualFreeEx(hproc, remote_pt, 0, MEM_RELEASE);
            let _ = windows::Win32::Foundation::CloseHandle(hproc);
            return None;
        }

        let mut positions = Vec::with_capacity(count);

        for i in 0..count {
            // SAFETY: Sending LVM_GETITEMPOSITION; remote_pt is valid remote memory.
            SendMessageW(
                listview,
                LVM_GETITEMPOSITION,
                Some(WPARAM(i)),
                Some(LPARAM(remote_pt as isize)),
            );

            let mut local_pt = Point::default();
            // SAFETY: Reading POINT from remote process memory.
            let _ = ReadProcessMemory(
                hproc,
                remote_pt,
                &mut local_pt as *mut Point as *mut _,
                std::mem::size_of::<Point>(),
                None,
            );

            // Pre-set RECT.left to LVIR_BOUNDS before sending LVM_GETITEMRECT
            let init_rect = Rect {
                left: LVIR_BOUNDS,
                ..Default::default()
            };
            // SAFETY: Writing initial RECT into remote memory.
            let _ = windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
                hproc,
                remote_rect,
                &init_rect as *const Rect as *const _,
                std::mem::size_of::<Rect>(),
                None,
            );

            // SAFETY: Sending LVM_GETITEMRECT; remote_rect initialized with LVIR_BOUNDS.
            SendMessageW(
                listview,
                LVM_GETITEMRECT,
                Some(WPARAM(i)),
                Some(LPARAM(remote_rect as isize)),
            );

            let mut local_rect = Rect::default();
            // SAFETY: Reading RECT from remote process memory.
            let _ = ReadProcessMemory(
                hproc,
                remote_rect,
                &mut local_rect as *mut Rect as *mut _,
                std::mem::size_of::<Rect>(),
                None,
            );

            positions.push(IconPosition {
                index: i,
                point: (local_pt.x, local_pt.y),
                bounds: (local_rect.left, local_rect.top, local_rect.right, local_rect.bottom),
            });
        }

        // SAFETY: Freeing previously allocated remote memory and closing process handle.
        let _ = VirtualFreeEx(hproc, remote_pt, 0, MEM_RELEASE);
        let _ = VirtualFreeEx(hproc, remote_rect, 0, MEM_RELEASE);
        let _ = windows::Win32::Foundation::CloseHandle(hproc);

        Some(DesktopSnapshot {
            item_count: count,
            positions,
            captured_at: std::time::Instant::now(),
        })
    }
}

/// Returns true if the given screen point hits an icon in the listview.
pub fn hit_test_icon(listview: HWND, screen_pt: (i32, i32)) -> bool {
    unsafe {
        // Convert screen coordinates to listview client coordinates
        let mut client_pt = windows::Win32::Foundation::POINT {
            x: screen_pt.0,
            y: screen_pt.1,
        };
        // SAFETY: ScreenToClient converts in-place; listview is a valid HWND.
        let _ = windows::Win32::Graphics::Gdi::ScreenToClient(listview, &mut client_pt);

        let mut pid: u32 = 0;
        // SAFETY: listview is valid; pid is a valid pointer to local stack memory.
        GetWindowThreadProcessId(listview, Some(&mut pid));
        if pid == 0 {
            return false;
        }

        // SAFETY: Opening explorer.exe process with VM operation permissions.
        let Ok(hproc) = OpenProcess(
            PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE,
            false,
            pid,
        ) else {
            return false;
        };

        // SAFETY: Allocating memory in the remote process for LVHITTESTINFO.
        let remote_buf = VirtualAllocEx(
            hproc,
            None,
            std::mem::size_of::<LvHitTestInfo>(),
            MEM_COMMIT,
            PAGE_READWRITE,
        );
        if remote_buf.is_null() {
            let _ = windows::Win32::Foundation::CloseHandle(hproc);
            return false;
        }

        let hit_info = LvHitTestInfo {
            pt: Point {
                x: client_pt.x,
                y: client_pt.y,
            },
            flags: 0,
            item: -1,
            sub_item: 0,
            group: 0,
        };

        // SAFETY: Writing LVHITTESTINFO into remote memory.
        let _ = windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
            hproc,
            remote_buf,
            &hit_info as *const LvHitTestInfo as *const _,
            std::mem::size_of::<LvHitTestInfo>(),
            None,
        );

        // SAFETY: Sending LVM_HITTEST to the listview with remote buffer.
        let result = SendMessageW(
            listview,
            LVM_HITTEST,
            None,
            Some(LPARAM(remote_buf as isize)),
        );

        // SAFETY: Freeing remote memory and closing process handle.
        let _ = VirtualFreeEx(hproc, remote_buf, 0, MEM_RELEASE);
        let _ = windows::Win32::Foundation::CloseHandle(hproc);

        // LVM_HITTEST returns the item index, or -1 if no item was hit
        result.0 != -1
    }
}

/// Restores icon positions from a snapshot using LVM_SETITEMPOSITION.
/// Coordinates are packed into lParam as MAKELPARAM(x, y).
pub fn restore_snapshot(listview: HWND, snapshot: &DesktopSnapshot) {
    for pos in &snapshot.positions {
        let lparam = ((pos.point.0 as u16 as u32) | ((pos.point.1 as u16 as u32) << 16)) as isize;
        // SAFETY: listview is valid; LVM_SETITEMPOSITION with packed coordinates
        // does not require cross-process memory.
        unsafe {
            SendMessageW(
                listview,
                LVM_SETITEMPOSITION,
                Some(WPARAM(pos.index)),
                Some(LPARAM(lparam)),
            );
        }
    }
}
