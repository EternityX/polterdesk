#![windows_subsystem = "windows"]

mod app_state;
mod desktop;
mod settings;
mod startup;
mod ui;
mod winapi_thread;

use app_state::{AppEvent, AppState};
use settings::Settings;
use std::sync::mpsc;
use windows::core::w;
use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
use windows::Win32::System::Threading::CreateMutexW;

fn main() {
    // SAFETY: CreateMutexW with no security attributes and a fixed name is safe.
    // We keep `_mutex` alive for the process lifetime to hold the lock.
    let _mutex = unsafe { CreateMutexW(None, true, w!("Polterdesk_SingleInstance")) };
    if unsafe { windows::Win32::Foundation::GetLastError() } == ERROR_ALREADY_EXISTS {
        return;
    }

    let mut settings = Settings::load();

    // Crash recovery: restore taskbar if it was left in app-controlled auto-hide
    match desktop::taskbar::restore_taskbar_if_needed(&mut settings) {
        Ok(true) => {
            let _ = settings.save();
        }
        Ok(false) => {}
        Err(e) => eprintln!("Taskbar crash recovery failed: {e}"),
    }

    // Channel for WinAPI thread -> GPUI thread communication
    let (gpui_tx, gpui_rx) = mpsc::channel::<AppEvent>();

    let state = AppState::new(settings.clone(), gpui_tx);

    // Spawn the WinAPI background thread (hotkey, tray, mouse hook)
    let _winapi_handle = winapi_thread::spawn(state.clone());

    // Start the GPUI application on the main thread
    gpui::Application::new().run(move |cx| {
        ui::theme::apply_theme(cx);

        // Open the settings window on startup
        let settings_window = {
            let guard = state.lock().unwrap();
            let s = guard.settings.clone();
            let tx = guard.gpui_tx.clone();
            drop(guard);
            ui::open_settings(cx, s, tx, state.clone())
        };

        // Poll the event channel on a 50ms interval
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;

                // Check if the window was minimized - hide to tray instead
                if let Some(handle) = settings_window {
                    let _ = cx.update(|cx| {
                        ui::check_minimize_to_tray(handle, cx);
                    });
                }

                // Drain all pending events
                while let Ok(event) = gpui_rx.try_recv() {
                    match event {
                        AppEvent::SettingsWindowRequested => {
                            // Re-show the existing window instead of opening a new one
                            if let Some(handle) = settings_window {
                                let _ = cx.update(|cx| {
                                    ui::show_window(handle, cx);
                                });
                            }
                        }
                        AppEvent::ExitRequested => {
                            // Restore icons and taskbar before quitting
                            {
                                let mut guard = state.lock().unwrap();
                                if guard.toggle_state == app_state::ToggleState::Hidden {
                                    if let Some(raw_hwnd) = guard.listview_hwnd {
                                        let listview =
                                            windows::Win32::Foundation::HWND(raw_hwnd as *mut _);
                                        desktop::toggle::show_icons(listview);
                                        if let Some(ref snapshot) = guard.snapshot {
                                            desktop::positions::restore_snapshot(
                                                listview, snapshot,
                                            );
                                        }
                                        guard.toggle_state = app_state::ToggleState::Visible;
                                        guard.snapshot = None;
                                    }
                                }
                                app_state::restore_taskbar_for_exit(&mut guard);
                            }
                            let _ = cx.update(|cx| {
                                cx.quit();
                            });
                            return Ok::<_, anyhow::Error>(());
                        }
                        AppEvent::HotkeyConflict(_binding) => {
                            if let Some(handle) = settings_window {
                                let _ = cx.update(|cx| {
                                    ui::show_window(handle, cx);
                                });
                            }
                        }
                        AppEvent::TaskbarHotkeyConflict(_binding) => {
                            if let Some(handle) = settings_window {
                                let _ = cx.update(|cx| {
                                    ui::show_window(handle, cx);
                                });
                            }
                        }
                        AppEvent::NativeToggleDetected
                        | AppEvent::ToggleTriggered
                        | AppEvent::TaskbarToggleTriggered => {}
                    }
                }
            }
        })
        .detach();
    });
}
