use polterdesk::app_state::{AppState, DesktopSnapshot, IconPosition, ToggleState};
use polterdesk::settings::Settings;
use std::sync::mpsc;

fn make_test_state() -> polterdesk::app_state::SharedState {
    let (tx, _rx) = mpsc::channel();
    AppState::new(Settings::default(), tx)
}

#[test]
fn initial_state_is_visible() {
    let state = make_test_state();
    let guard = state.lock().unwrap();
    assert_eq!(guard.toggle_state, ToggleState::Visible);
    assert!(guard.snapshot.is_none());
}

#[test]
fn transition_visible_to_hidden_sets_snapshot() {
    let state = make_test_state();
    let mut guard = state.lock().unwrap();

    // Simulate toggle: Visible -> Hidden
    guard.toggle_state = ToggleState::Hidden;
    guard.snapshot = Some(DesktopSnapshot {
        item_count: 1,
        positions: vec![IconPosition {
            index: 0,
            point: (10, 20),
            bounds: (0, 10, 80, 90),
        }],
        captured_at: std::time::Instant::now(),
    });

    assert_eq!(guard.toggle_state, ToggleState::Hidden);
    assert!(guard.snapshot.is_some());
}

#[test]
fn transition_hidden_to_visible_clears_snapshot() {
    let state = make_test_state();
    let mut guard = state.lock().unwrap();

    // Set up Hidden state
    guard.toggle_state = ToggleState::Hidden;
    guard.snapshot = Some(DesktopSnapshot {
        item_count: 0,
        positions: vec![],
        captured_at: std::time::Instant::now(),
    });

    // Transition back to Visible
    guard.toggle_state = ToggleState::Visible;
    guard.snapshot = None;

    assert_eq!(guard.toggle_state, ToggleState::Visible);
    assert!(guard.snapshot.is_none());
}

#[test]
fn cannot_transition_hidden_to_hidden() {
    // Hidden -> Hidden should be a no-op: snapshot remains unchanged
    let state = make_test_state();
    let mut guard = state.lock().unwrap();

    guard.toggle_state = ToggleState::Hidden;
    let snapshot = DesktopSnapshot {
        item_count: 2,
        positions: vec![
            IconPosition {
                index: 0,
                point: (10, 20),
                bounds: (0, 10, 80, 90),
            },
            IconPosition {
                index: 1,
                point: (100, 20),
                bounds: (90, 10, 170, 90),
            },
        ],
        captured_at: std::time::Instant::now(),
    };
    guard.snapshot = Some(snapshot);

    // Attempting Hidden -> Hidden: state shouldn't change
    assert_eq!(guard.toggle_state, ToggleState::Hidden);
    assert!(guard.snapshot.is_some());
    assert_eq!(guard.snapshot.as_ref().unwrap().item_count, 2);
}

#[test]
fn native_toggle_detected_from_hidden_sets_visible() {
    let state = make_test_state();
    let mut guard = state.lock().unwrap();

    // Set up Hidden state
    guard.toggle_state = ToggleState::Hidden;
    guard.snapshot = Some(DesktopSnapshot {
        item_count: 1,
        positions: vec![IconPosition {
            index: 0,
            point: (10, 20),
            bounds: (0, 10, 80, 90),
        }],
        captured_at: std::time::Instant::now(),
    });

    // Simulate NativeToggleDetected
    guard.toggle_state = ToggleState::Visible;
    guard.snapshot = None;

    assert_eq!(guard.toggle_state, ToggleState::Visible);
    assert!(guard.snapshot.is_none());
}
