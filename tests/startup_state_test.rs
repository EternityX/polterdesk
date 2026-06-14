//! Tests for startup state detection — "adopt reality" reconciliation.
//!
//! At launch the app must derive its world-model from the *actual* OS state
//! instead of hardcoded constants. `resolve_initial_state` is the pure decision
//! core: given the observed facts (icons visible? taskbar auto-hiding?) plus the
//! persisted `taskbar_original_state`, it decides the initial app state without
//! changing the desktop. These tests pin every combination.

use polterdesk::app_state::{resolve_initial_state, ToggleState};

#[test]
fn icons_visible_taskbar_normal_no_record_is_visible() {
    let s = resolve_initial_state(true, false, None);
    assert_eq!(s.toggle_state, ToggleState::Visible);
    assert!(!s.taskbar_hidden);
    assert_eq!(s.taskbar_original_state, None);
    assert!(!s.clear_persisted_original);
}

#[test]
fn icons_hidden_is_adopted_as_hidden() {
    // The core bug: icons are really hidden (e.g. SW_HIDE survived a crash) but
    // the old code assumed Visible. We must adopt Hidden.
    let s = resolve_initial_state(false, false, None);
    assert_eq!(s.toggle_state, ToggleState::Hidden);
    assert!(!s.taskbar_hidden);
}

#[test]
fn taskbar_autohide_without_record_is_user_preference_not_claimed() {
    // Auto-hide is on but we have no record of enabling it → it's the user's own
    // preference. We must NOT claim control (else we'd later "restore" it off).
    let s = resolve_initial_state(true, true, None);
    assert_eq!(s.toggle_state, ToggleState::Visible);
    assert!(!s.taskbar_hidden);
    assert_eq!(s.taskbar_original_state, None);
    assert!(!s.clear_persisted_original);
}

#[test]
fn we_still_control_taskbar_when_autohide_persisted_and_active() {
    // We enabled auto-hide last run (persisted Some(2)) and it's still on → we
    // resume control and keep the original for exit restoration. Desktop unchanged.
    let s = resolve_initial_state(false, true, Some(2));
    assert_eq!(s.toggle_state, ToggleState::Hidden);
    assert!(s.taskbar_hidden);
    assert_eq!(s.taskbar_original_state, Some(2));
    assert!(!s.clear_persisted_original);
}

#[test]
fn taskbar_record_dropped_when_autohide_turned_off_externally() {
    // We had control, but auto-hide is now off → user/Explorer reset it. Drop the
    // stale record so we don't think we're still in control (the stuck-state bug).
    let s = resolve_initial_state(true, false, Some(2));
    assert_eq!(s.toggle_state, ToggleState::Visible);
    assert!(!s.taskbar_hidden);
    assert_eq!(s.taskbar_original_state, None);
    assert!(s.clear_persisted_original);
}

#[test]
fn original_state_zero_is_preserved_when_still_controlling() {
    // A persisted original of 0 (bare, no flags) must round-trip, not be confused
    // with "no record".
    let s = resolve_initial_state(false, true, Some(0));
    assert!(s.taskbar_hidden);
    assert_eq!(s.taskbar_original_state, Some(0));
    assert!(!s.clear_persisted_original);
}

#[test]
fn icons_and_taskbar_resolved_independently() {
    // Icons may be visible while we still control an auto-hidden taskbar (e.g. a
    // native "show icons" happened mid-session, then a crash). Adopt both truthfully;
    // the combined-mode toggle path self-heals from here.
    let s = resolve_initial_state(true, true, Some(2));
    assert_eq!(s.toggle_state, ToggleState::Visible);
    assert!(s.taskbar_hidden);
    assert_eq!(s.taskbar_original_state, Some(2));
}
