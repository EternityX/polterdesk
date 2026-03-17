use windows::Win32::System::Registry::{
    RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "Polterdesk";

/// Sets or removes the application from Windows startup (HKCU\...\Run).
pub fn set_startup(enabled: bool) {
    unsafe {
        let subkey = windows::core::HSTRING::from(RUN_KEY);
        let mut hkey = windows::Win32::System::Registry::HKEY::default();

        // SAFETY: Opening a well-known registry key path with write access.
        let result = RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, None, KEY_SET_VALUE, &mut hkey);
        if result.is_err() {
            eprintln!("Failed to open registry key: {:?}", result);
            return;
        }

        let value_name = windows::core::HSTRING::from(VALUE_NAME);

        if enabled {
            let exe_path = match std::env::current_exe() {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(e) => {
                    eprintln!("Failed to get exe path: {e}");
                    let _ = windows::Win32::Foundation::CloseHandle(
                        windows::Win32::Foundation::HANDLE(hkey.0),
                    );
                    return;
                }
            };

            let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
            let data = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);

            // SAFETY: Writing a REG_SZ value with valid data to an open registry key.
            let _ = RegSetValueExW(hkey, &value_name, None, REG_SZ, Some(data));
        } else {
            // SAFETY: Deleting a registry value from an open key.
            let _ = RegDeleteValueW(hkey, &value_name);
        }

        let _ = windows::Win32::Foundation::CloseHandle(windows::Win32::Foundation::HANDLE(hkey.0));
    }
}
