//! Boot-time readiness gating.
//!
//! When the app is launched at logon via the HKCU Run key, parts of the Windows
//! graphics/shell stack are not necessarily ready yet. In particular,
//! gpui::Application::new() constructs WindowsPlatform::new(), which creates a
//! shared DirectWrite factory (DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED))
//! and enumerates the system font collection (GetSystemFontCollection). Both of
//! those depend on the Windows Font Cache Service (FontCache), which is often
//! still StartPending in the first seconds of a session.
//!
//! When they fail, GPUI calls show_error("Failed to launch", ...) a
//! MB_ICONERROR | MB_SYSTEMMODAL message box — and then .unwrap() panics, so the
//! process aborts and the app never starts. That is the "Windows error message box at
//! startup" symptom.
//!
//! To remove the race we gate GPUI construction on the *same* DirectWrite calls
//! succeeding here first, with backoff. On a manual launch the font subsystem is
//! already up, so the probe succeeds on the first attempt and we return immediately.

use std::time::Duration;

/// Number of probe attempts before giving up (120 × 250ms ≈ 30s).
const FONT_WAIT_ATTEMPTS: u32 = 120;
/// Delay between probe attempts.
const FONT_WAIT_DELAY: Duration = Duration::from_millis(250);

/// Polls probe until it returns Ok, sleeping delay between attempts, up to
/// max_attempts times.
///
/// Returns Ok(attempt_count) on the first success, or Err(last_error) if every
/// attempt failed (or max_attempts is 0). This is a pure control-flow harness with
/// the OS call injected, so it can be unit-tested deterministically.
pub fn poll_until_ready<F>(max_attempts: u32, delay: Duration, mut probe: F) -> Result<u32, String>
where
    F: FnMut() -> Result<(), String>,
{
    let mut last_error = String::from("probe never ran");
    for attempt in 1..=max_attempts {
        match probe() {
            Ok(()) => return Ok(attempt),
            Err(e) => {
                last_error = e;
                if attempt < max_attempts {
                    std::thread::sleep(delay);
                }
            }
        }
    }
    Err(last_error)
}

/// Probes the DirectWrite font subsystem once, mirroring the Font-Cache-dependent
/// calls GPUI makes during WindowsPlatform::new(): create a shared factory and
/// enumerate the system font collection.
fn probe_font_subsystem() -> Result<(), String> {
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, DWRITE_FACTORY_TYPE_SHARED,
    };

    // SAFETY: DWriteCreateFactory returns an owned COM interface on success.
    // GetSystemFontCollection is a read-only query; we pass a valid out-pointer and
    // only read the returned interface. No raw pointers escape this block.
    unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
            .map_err(|e| format!("DWriteCreateFactory(shared) failed: {e}"))?;

        let mut collection: Option<IDWriteFontCollection> = None;
        factory
            .GetSystemFontCollection(&mut collection, false)
            .map_err(|e| format!("GetSystemFontCollection failed: {e}"))?;

        match collection {
            // A populated collection means the Font Cache Service answered.
            Some(c) if c.GetFontFamilyCount() > 0 => Ok(()),
            Some(_) => Err("system font collection is empty".to_string()),
            None => Err("system font collection was null".to_string()),
        }
    }
}

/// Blocks until the DirectWrite font subsystem is ready, or a ~30s timeout elapses.
///
/// Must be called before gpui::Application::new(). See the module docs for why.
/// If the timeout is hit we proceed anyway (GPUI may then show its own error box),
/// having recorded the last error to the startup log for diagnosis.
pub fn wait_for_font_subsystem() {
    match poll_until_ready(FONT_WAIT_ATTEMPTS, FONT_WAIT_DELAY, probe_font_subsystem) {
        // The common case (manual launch / fast boot): ready immediately, no log noise.
        Ok(1) => {}
        Ok(attempts) => {
            let waited_ms = (attempts - 1) * FONT_WAIT_DELAY.as_millis() as u32;
            log_startup(&format!(
                "Font subsystem ready after {attempts} attempt(s) (~{waited_ms}ms wait)."
            ));
        }
        Err(last_error) => {
            let timeout_s = FONT_WAIT_ATTEMPTS as u128 * FONT_WAIT_DELAY.as_millis() / 1000;
            log_startup(&format!(
                "Font subsystem NOT ready after ~{timeout_s}s; constructing GPUI anyway. \
                 Last error: {last_error}"
            ));
        }
    }
}

/// Appends a timestamped line to %APPDATA%\Polterdesk\startup.log. Best-effort;
/// any failure is ignored (we must never block startup on logging).
fn log_startup(msg: &str) {
    use std::io::Write;

    let Ok(appdata) = std::env::var("APPDATA") else {
        return;
    };
    let dir = std::path::PathBuf::from(appdata).join("Polterdesk");
    let _ = std::fs::create_dir_all(&dir);

    let unix_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("startup.log"))
    {
        let _ = writeln!(f, "[{unix_secs}] {msg}");
    }
}
