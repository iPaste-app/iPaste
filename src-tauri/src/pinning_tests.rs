use super::*;

struct Fixture(Store);

impl Fixture {
    fn new() -> Self {
        let store = Store {
            db_path: std::env::temp_dir().join(format!("ipaste-pinning-{}.sqlite", new_id())),
        };
        store.migrate(&store.connect().unwrap()).unwrap();
        Self(store)
    }

    fn clip(&self, id: &str, pinned: bool, captured_at: &str) {
        self.0.connect().unwrap().execute(
            "INSERT INTO clips (id, clip_type, content_hash, preview_text, text, last_captured_at, is_pinned)
             VALUES (?1, 'text', ?1, ?1, ?1, ?2, ?3)",
            params![id, captured_at, pinned],
        ).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0.db_path);
        let _ = fs::remove_file(self.0.db_path.with_extension("sqlite-wal"));
        let _ = fs::remove_file(self.0.db_path.with_extension("sqlite-shm"));
    }
}

#[test]
fn pinned_history_is_sorted_before_paging_and_search_filters_both_groups() {
    let fixture = Fixture::new();
    for index in 0..25 {
        fixture.clip(&format!("pin-{index:02}"), true, "2000-01-01T00:00:00Z");
        fixture.clip(&format!("normal-{index:02}"), false, "2026-09-18T00:00:00Z");
    }
    let first = fixture.0.list_clips(0, 20, String::new()).unwrap();
    assert!(first.has_more);
    assert_eq!(first.total_count, 50);
    assert!(first.clips.iter().all(|item| item.is_pinned));
    assert_eq!(first.clips[0].id, "pin-00");
    let second = fixture.0.list_clips(20, 20, String::new()).unwrap();
    assert_eq!(second.clips[0].id, "pin-20");
    assert_eq!(second.clips[5].id, "normal-00");
    let third = fixture.0.list_clips(40, 20, String::new()).unwrap();
    assert!(!third.has_more);
    let ids: std::collections::HashSet<_> = first
        .clips
        .iter()
        .chain(&second.clips)
        .chain(&third.clips)
        .map(|item| &item.id)
        .collect();
    assert_eq!(ids.len(), 50);
    let matches = fixture.0.list_clips(0, 20, "-04".into()).unwrap();
    assert_eq!(matches.total_count, 2);
    assert_eq!(matches.clips[0].id, "pin-04");
    assert_eq!(matches.clips[1].id, "normal-04");
}

#[test]
fn pins_persist_and_remain_independent_between_history_and_category_snapshots() {
    let fixture = Fixture::new();
    fixture.clip("history", false, "2000-01-01T00:00:00Z");
    fixture
        .0
        .set_clip_pinned("history".into(), "history".into(), true)
        .unwrap();
    let category = fixture
        .0
        .create_category("Saved".into(), "#0D9488".into())
        .unwrap();
    let saved = fixture
        .0
        .add_clip_to_category("history".into(), category.id)
        .unwrap();
    assert!(!saved.is_pinned);
    fixture
        .0
        .set_clip_pinned(saved.id.clone(), "category".into(), true)
        .unwrap();
    fixture
        .0
        .set_clip_pinned("history".into(), "history".into(), false)
        .unwrap();
    let reopened = Store::new(fixture.0.db_path.clone()).unwrap();
    let conn = reopened.connect().unwrap();
    let saved = reopened
        .get_category_item_with_conn(&conn, &saved.id)
        .unwrap();
    assert!(saved.is_pinned);
    assert_eq!(saved.sync_state, "local");
    assert!(
        !reopened
            .get_clip_with_conn(&conn, "history")
            .unwrap()
            .is_pinned
    );
}

#[test]
fn pruning_keeps_pins_until_unpinned_and_manual_delete_still_works() {
    let fixture = Fixture::new();
    fixture.clip("pin", true, "2000-01-01T00:00:00Z");
    fixture.clip("expired", false, "2000-01-01T00:00:00Z");
    let conn = fixture.0.connect().unwrap();
    fixture.0.prune_expired_with_conn(&conn, 30).unwrap();
    assert_eq!(fixture.0.clip_total_count_with_conn(&conn).unwrap(), 1);
    assert!(
        fixture
            .0
            .get_clip_with_conn(&conn, "pin")
            .unwrap()
            .is_pinned
    );
    fixture
        .0
        .set_clip_pinned("pin".into(), "history".into(), false)
        .unwrap();
    fixture.0.prune_expired_with_conn(&conn, 30).unwrap();
    assert_eq!(fixture.0.clip_total_count_with_conn(&conn).unwrap(), 0);
    fixture.clip("manual", true, "2000-01-01T00:00:00Z");
    fixture.0.delete_clip("manual".into()).unwrap();
    assert_eq!(fixture.0.clip_total_count_with_conn(&conn).unwrap(), 0);
}

#[test]
fn recapture_keeps_the_pin_without_inserting_a_duplicate() {
    let fixture = Fixture::new();
    fixture.clip("pin", true, "2000-01-01T00:00:00Z");
    let (item, count, inserted) = fixture
        .0
        .insert_captured_item(CapturedClipboardItem {
            clip_type: "text".into(),
            content_hash: "pin".into(),
            preview_text: "pin".into(),
            text: "pin".into(),
            image_bytes: None,
        })
        .unwrap()
        .unwrap();
    assert!(item.is_pinned);
    assert!(!inserted);
    assert_eq!(count, 1);
    assert_ne!(item.last_captured_at, "2000-01-01T00:00:00Z");
}

#[test]
fn category_dragging_preserves_the_underlying_position_of_the_pinned_item() {
    let fixture = Fixture::new();
    let category = fixture
        .0
        .create_category("Saved".into(), "#0D9488".into())
        .unwrap();
    let mut saved = Vec::new();
    for id in ["first", "pin", "last"] {
        fixture.clip(id, false, "2026-09-18T00:00:00Z");
        saved.push(
            fixture
                .0
                .add_clip_to_category(id.into(), category.id.clone())
                .unwrap(),
        );
    }
    let initial: Vec<_> = saved.iter().map(|item| item.id.clone()).collect();
    fixture
        .0
        .reorder_category_items(category.id.clone(), initial.clone())
        .unwrap();
    fixture
        .0
        .set_clip_pinned(initial[1].clone(), "category".into(), true)
        .unwrap();
    let order = vec![initial[1].clone(), initial[2].clone(), initial[0].clone()];
    let items = fixture
        .0
        .reorder_category_items(category.id.clone(), order)
        .unwrap();
    assert_eq!(items[0].id, initial[1]);
    assert_eq!(items[0].sort_order, 1);
    assert!(fixture
        .0
        .reorder_category_items(category.id, initial.clone())
        .is_err());
    fixture
        .0
        .set_clip_pinned(initial[1].clone(), "category".into(), false)
        .unwrap();
    let items = fixture
        .0
        .list_category_items_with_conn(&fixture.0.connect().unwrap())
        .unwrap();
    assert_eq!(
        items.iter().map(|item| &item.id).collect::<Vec<_>>(),
        vec![&initial[2], &initial[1], &initial[0]]
    );
}

#[test]
fn sync_serializes_pins_and_an_older_response_does_not_overwrite_local_changes() {
    let fixture = Fixture::new();
    fixture.clip("clip", false, "2026-09-18T00:00:00Z");
    let category = fixture
        .0
        .create_category("Saved".into(), "#0D9488".into())
        .unwrap();
    let mut remote = fixture
        .0
        .add_clip_to_category("clip".into(), category.id)
        .unwrap();
    remote.updated_at = "2026-09-18T10:00:00.100Z".into();
    let conn = fixture.0.connect().unwrap();
    conn.execute("UPDATE category_items SET is_pinned = 1, updated_at = '2026-09-18T10:00:00.900Z' WHERE id = ?1",
        params![remote.id]).unwrap();
    let payload = CloudPushPayload {
        categories: vec![],
        category_items: fixture
            .0
            .list_syncable_category_items_with_conn(&conn)
            .unwrap(),
        deleted_category_ids: vec![],
        deleted_category_item_ids: vec![],
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap()["categoryItems"][0]["isPinned"],
        true
    );
    fixture
        .0
        .merge_cloud_snapshot_with_conn(
            &conn,
            CloudSnapshot {
                categories: vec![],
                category_items: vec![remote.clone()],
                deleted_category_ids: vec![],
                deleted_category_item_ids: vec![],
            },
        )
        .unwrap();
    let local = fixture
        .0
        .get_category_item_with_conn(&conn, &remote.id)
        .unwrap();
    assert!(local.is_pinned);
    assert_eq!(local.sync_state, "local");
    remote.updated_at = "2026-09-18T10:00:01Z".into();
    fixture
        .0
        .merge_cloud_snapshot_with_conn(
            &conn,
            CloudSnapshot {
                categories: vec![],
                category_items: vec![remote.clone()],
                deleted_category_ids: vec![],
                deleted_category_item_ids: vec![],
            },
        )
        .unwrap();
    let local = fixture
        .0
        .get_category_item_with_conn(&conn, &remote.id)
        .unwrap();
    assert!(!local.is_pinned);
    assert_eq!(local.sync_state, "synced");
}

#[test]
fn fourth_pin_is_first_recapture_is_stable_and_repin_returns_to_front() {
    let fixture = Fixture::new();
    for id in ["a", "b", "c", "d"] {
        fixture.clip(id, false, "2026-09-18T00:00:00Z");
        fixture
            .0
            .set_clip_pinned(id.into(), "history".into(), true)
            .unwrap();
    }
    let ids = || {
        fixture
            .0
            .list_clips(0, 20, String::new())
            .unwrap()
            .clips
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(), ["d", "c", "b", "a"]);
    fixture
        .0
        .insert_captured_item(CapturedClipboardItem {
            clip_type: "text".into(),
            content_hash: "a".into(),
            preview_text: "a".into(),
            text: "a".into(),
            image_bytes: None,
        })
        .unwrap();
    assert_eq!(ids(), ["d", "c", "b", "a"]);
    // A repeated request is idempotent; only a new false -> true transition moves it.
    fixture
        .0
        .set_clip_pinned("a".into(), "history".into(), true)
        .unwrap();
    assert_eq!(ids(), ["d", "c", "b", "a"]);
    fixture
        .0
        .set_clip_pinned("a".into(), "history".into(), false)
        .unwrap();
    assert_eq!(
        fixture
            .0
            .get_clip_with_conn(&fixture.0.connect().unwrap(), "a")
            .unwrap()
            .pin_order,
        None
    );
    fixture
        .0
        .set_clip_pinned("a".into(), "history".into(), true)
        .unwrap();
    assert_eq!(ids(), ["a", "d", "c", "b"]);
    let reopened = Store::new(fixture.0.db_path.clone()).unwrap();
    assert_eq!(
        reopened.list_clips(0, 20, String::new()).unwrap().clips[0].id,
        "a"
    );
}

#[test]
fn newest_pin_precedes_all_previous_pins_across_pages() {
    let fixture = Fixture::new();
    for index in 0..25 {
        let id = format!("pin-{index:02}");
        fixture.clip(&id, false, "2026-09-18T00:00:00Z");
        fixture
            .0
            .set_clip_pinned(id, "history".into(), true)
            .unwrap();
    }
    let first = fixture.0.list_clips(0, 20, String::new()).unwrap();
    let second = fixture.0.list_clips(20, 20, String::new()).unwrap();
    assert_eq!(first.clips[0].id, "pin-24");
    assert_eq!(first.clips[19].id, "pin-05");
    assert_eq!(second.clips[0].id, "pin-04");
    assert_eq!(second.clips[4].id, "pin-00");
    assert!(!second.has_more);
}

#[test]
fn migration_keeps_existing_pin_order_and_is_idempotent() {
    let fixture = Fixture::new();
    let conn = fixture.0.connect().unwrap();
    conn.execute_batch(
        "DROP INDEX idx_clips_pin_order;
        ALTER TABLE clips DROP COLUMN pin_order;
        ALTER TABLE category_items DROP COLUMN pin_order;",
    )
    .unwrap();
    fixture.clip("old", true, "2000-01-01T00:00:00Z");
    fixture.clip("recent", true, "2026-09-18T00:00:00Z");
    fixture.clip("normal", false, "2026-09-19T00:00:00Z");
    pinning::migrate(&conn).unwrap();
    let first = fixture.0.list_clips(0, 20, String::new()).unwrap();
    assert_eq!(
        first
            .clips
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["recent", "old", "normal"]
    );
    let orders = first
        .clips
        .iter()
        .map(|item| item.pin_order)
        .collect::<Vec<_>>();
    assert_eq!(orders, [Some(2), Some(1), None]);
    pinning::migrate(&conn).unwrap();
    let second = fixture.0.list_clips(0, 20, String::new()).unwrap();
    assert_eq!(
        second
            .clips
            .iter()
            .map(|item| item.pin_order)
            .collect::<Vec<_>>(),
        orders
    );
    fixture
        .0
        .set_clip_pinned("normal".into(), "history".into(), true)
        .unwrap();
    assert_eq!(
        fixture.0.list_clips(0, 20, String::new()).unwrap().clips[0].id,
        "normal"
    );
}

#[test]
fn category_pin_dragging_does_not_change_unpin_positions() {
    let fixture = Fixture::new();
    let category = fixture
        .0
        .create_category("Saved".into(), "#0D9488".into())
        .unwrap();
    let mut ids = Vec::new();
    for id in ["a", "b", "c", "d"] {
        fixture.clip(id, false, "2026-09-18T00:00:00Z");
        ids.push(
            fixture
                .0
                .add_clip_to_category(id.into(), category.id.clone())
                .unwrap()
                .id,
        );
    }
    fixture
        .0
        .reorder_category_items(category.id.clone(), ids.clone())
        .unwrap();
    for id in &ids {
        fixture
            .0
            .set_clip_pinned(id.clone(), "category".into(), true)
            .unwrap();
    }
    let items = fixture.0.list_category_items().unwrap();
    assert_eq!(
        items.iter().map(|item| &item.id).collect::<Vec<_>>(),
        ids.iter().rev().collect::<Vec<_>>()
    );
    let order = vec![
        ids[1].clone(),
        ids[3].clone(),
        ids[0].clone(),
        ids[2].clone(),
    ];
    let items = fixture
        .0
        .reorder_category_items(category.id, order.clone())
        .unwrap();
    assert_eq!(
        items.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        order
    );
    for id in &ids {
        fixture
            .0
            .set_clip_pinned(id.clone(), "category".into(), false)
            .unwrap();
    }
    let items = fixture.0.list_category_items().unwrap();
    assert_eq!(
        items.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        ids
    );
}

#[test]
fn cloud_roundtrip_includes_pin_order_and_legacy_payloads_preserve_local_order() {
    let fixture = Fixture::new();
    fixture.clip("clip", false, "2026-09-18T00:00:00Z");
    let category = fixture
        .0
        .create_category("Saved".into(), "#0D9488".into())
        .unwrap();
    let saved = fixture
        .0
        .add_clip_to_category("clip".into(), category.id)
        .unwrap();
    let mut remote = saved.clone();
    remote.is_pinned = true;
    remote.pin_order = Some(42);
    remote.updated_at = "2099-01-01T00:00:00Z".into();
    let conn = fixture.0.connect().unwrap();
    let merge = |item| {
        fixture
            .0
            .merge_cloud_snapshot_with_conn(
                &conn,
                CloudSnapshot {
                    categories: vec![],
                    category_items: vec![item],
                    deleted_category_ids: vec![],
                    deleted_category_item_ids: vec![],
                },
            )
            .unwrap()
    };
    merge(remote.clone());
    let local = fixture
        .0
        .get_category_item_with_conn(&conn, &saved.id)
        .unwrap();
    assert_eq!(local.pin_order, Some(42));
    assert_eq!(serde_json::to_value(local).unwrap()["pinOrder"], 42);
    let mut legacy = serde_json::to_value(&remote).unwrap();
    legacy.as_object_mut().unwrap().remove("pinOrder");
    merge(serde_json::from_value(legacy).unwrap());
    assert_eq!(
        fixture
            .0
            .get_category_item_with_conn(&conn, &saved.id)
            .unwrap()
            .pin_order,
        Some(42)
    );
    remote.is_pinned = false;
    remote.pin_order = None;
    merge(remote);
    assert_eq!(
        fixture
            .0
            .get_category_item_with_conn(&conn, &saved.id)
            .unwrap()
            .pin_order,
        None
    );
}
