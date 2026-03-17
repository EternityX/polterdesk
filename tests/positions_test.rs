use polterdesk::app_state::{DesktopSnapshot, IconPosition};

/// Helper: check if a point is inside an icon's bounds rectangle.
fn point_in_bounds(pt: (i32, i32), bounds: (i32, i32, i32, i32)) -> bool {
    let (x, y) = pt;
    let (left, top, right, bottom) = bounds;
    x >= left && x <= right && y >= top && y <= bottom
}

#[test]
fn hit_test_inside_bounds() {
    let bounds = (100, 100, 200, 200);
    assert!(point_in_bounds((150, 150), bounds));
}

#[test]
fn hit_test_outside_bounds() {
    let bounds = (100, 100, 200, 200);
    assert!(!point_in_bounds((50, 50), bounds));
    assert!(!point_in_bounds((250, 150), bounds));
    assert!(!point_in_bounds((150, 250), bounds));
}

#[test]
fn hit_test_edge_pixel_is_inside() {
    let bounds = (100, 100, 200, 200);
    // All four edges
    assert!(point_in_bounds((100, 100), bounds)); // top-left
    assert!(point_in_bounds((200, 200), bounds)); // bottom-right
    assert!(point_in_bounds((100, 200), bounds)); // bottom-left
    assert!(point_in_bounds((200, 100), bounds)); // top-right
}

#[test]
fn snapshot_with_zero_items_is_valid() {
    let snapshot = DesktopSnapshot {
        item_count: 0,
        positions: vec![],
        captured_at: std::time::Instant::now(),
    };
    assert_eq!(snapshot.item_count, 0);
    assert!(snapshot.positions.is_empty());
}

#[test]
fn snapshot_positions_match_item_count() {
    let positions = vec![
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
    ];
    let snapshot = DesktopSnapshot {
        item_count: 2,
        positions: positions.clone(),
        captured_at: std::time::Instant::now(),
    };
    assert_eq!(snapshot.positions.len(), snapshot.item_count);
}
