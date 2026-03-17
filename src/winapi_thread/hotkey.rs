use crate::settings::{HotkeyBinding, Modifier, VirtualKey};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    RegisterHotKey, UnregisterHotKey,
};

const HOTKEY_ID: i32 = 1;
pub const TASKBAR_HOTKEY_ID: i32 = 2;

#[derive(Debug)]
pub enum HotkeyError {
    Conflict,
    #[allow(dead_code)]
    Other(windows::core::Error),
}

/// Maps modifier enums to Win32 HOT_KEY_MODIFIERS flags.
pub fn make_mods(binding: &HotkeyBinding) -> HOT_KEY_MODIFIERS {
    let mut mods = MOD_NOREPEAT;
    for m in &binding.modifiers {
        mods |= match m {
            Modifier::Ctrl => MOD_CONTROL,
            Modifier::Alt => MOD_ALT,
            Modifier::Shift => MOD_SHIFT,
            Modifier::Win => MOD_WIN,
        };
    }
    mods
}

/// Maps a VirtualKey to a Win32 virtual key code.
fn vk_code(key: &VirtualKey) -> u32 {
    match key {
        VirtualKey::Char(c) => {
            // For A-Z and 0-9, the virtual key code is the uppercase ASCII value
            c.to_ascii_uppercase() as u32
        }
        VirtualKey::F(n) => {
            // F1 = 0x70, F2 = 0x71, etc.
            0x6F + (*n as u32)
        }
    }
}

/// Registers the global hotkey. Returns Err(Conflict) if already registered by another app.
pub fn register(hwnd: HWND, binding: &HotkeyBinding) -> Result<(), HotkeyError> {
    let mods = make_mods(binding);
    let vk = vk_code(&binding.key);

    // SAFETY: hwnd is a valid window handle. RegisterHotKey associates the hotkey
    // with this window's message queue. If the hotkey is already registered by
    // another application, the call fails.
    let result = unsafe { RegisterHotKey(Some(hwnd), HOTKEY_ID, mods, vk) };

    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            // ERROR_HOTKEY_ALREADY_REGISTERED = 1409
            if e.code().0 as u32 == 0x80070581 {
                Err(HotkeyError::Conflict)
            } else {
                Err(HotkeyError::Other(e))
            }
        }
    }
}

/// Unregisters the global hotkey.
pub fn unregister(hwnd: HWND) {
    // SAFETY: Unregistering the hotkey associated with this window.
    let _ = unsafe { UnregisterHotKey(Some(hwnd), HOTKEY_ID) };
}

/// Unregisters and re-registers with a new binding.
pub fn reregister(hwnd: HWND, binding: &HotkeyBinding) -> Result<(), HotkeyError> {
    unregister(hwnd);
    register(hwnd, binding)
}

/// Registers the taskbar hotkey.
pub fn register_taskbar(hwnd: HWND, binding: &HotkeyBinding) -> Result<(), HotkeyError> {
    let mods = make_mods(binding);
    let vk = vk_code(&binding.key);

    // SAFETY: RegisterHotKey associates the taskbar hotkey with TASKBAR_HOTKEY_ID.
    let result = unsafe { RegisterHotKey(Some(hwnd), TASKBAR_HOTKEY_ID, mods, vk) };

    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            if e.code().0 as u32 == 0x80070581 {
                Err(HotkeyError::Conflict)
            } else {
                Err(HotkeyError::Other(e))
            }
        }
    }
}

/// Unregisters the taskbar hotkey.
pub fn unregister_taskbar(hwnd: HWND) {
    // SAFETY: Unregistering the taskbar hotkey associated with TASKBAR_HOTKEY_ID.
    let _ = unsafe { UnregisterHotKey(Some(hwnd), TASKBAR_HOTKEY_ID) };
}

/// Unregisters and re-registers the taskbar hotkey with a new binding.
pub fn reregister_taskbar(hwnd: HWND, binding: &HotkeyBinding) -> Result<(), HotkeyError> {
    unregister_taskbar(hwnd);
    register_taskbar(hwnd, binding)
}
