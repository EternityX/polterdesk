#[cfg(target_os = "windows")]
mod windows_tests {
    use polterdesk::desktop::{finder, positions, toggle};

    #[test]
    fn toggle_cycle_preserves_positions() {
        // Step 1: Find the desktop listview
        let listview = finder::find_desktop_listview();
        assert!(
            listview.is_some(),
            "Desktop listview not found - is explorer.exe running?"
        );
        let listview = listview.unwrap();

        // Step 2: Save a snapshot - should have at least one icon
        let snapshot = positions::save_snapshot(listview);
        assert!(snapshot.is_some(), "Failed to save desktop snapshot");
        let snapshot = snapshot.unwrap();
        assert!(
            snapshot.item_count > 0,
            "Desktop has no icons - test requires at least one icon"
        );

        // Step 3: Hide icons
        toggle::hide_icons(listview);
        assert!(
            !toggle::is_visible(listview),
            "Listview should be hidden after hide_icons"
        );

        // Step 4: Show icons and restore positions
        toggle::show_icons(listview);
        positions::restore_snapshot(listview, &snapshot);
        assert!(
            toggle::is_visible(listview),
            "Listview should be visible after show_icons"
        );

        // Step 5: Verify positions are restored (within 2px tolerance)
        let restored_snapshot = positions::save_snapshot(listview);
        assert!(
            restored_snapshot.is_some(),
            "Failed to save restored snapshot"
        );
        let restored = restored_snapshot.unwrap();

        let check_count = snapshot.item_count.min(restored.item_count);
        for i in 0..check_count {
            let original = &snapshot.positions[i];
            let restored_pos = &restored.positions[i];
            let dx = (original.point.0 - restored_pos.point.0).abs();
            let dy = (original.point.1 - restored_pos.point.1).abs();
            assert!(
                dx <= 2 && dy <= 2,
                "Position mismatch for icon {}: original ({}, {}), restored ({}, {})",
                i,
                original.point.0,
                original.point.1,
                restored_pos.point.0,
                restored_pos.point.1,
            );
        }
    }
}
