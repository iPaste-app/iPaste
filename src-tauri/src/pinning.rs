use rusqlite::Connection;

use super::add_column_if_missing;

pub(super) fn migrate(conn: &Connection) -> Result<(), String> {
    add_column_if_missing(conn, "clips", "pin_order", "INTEGER")?;
    add_column_if_missing(conn, "category_items", "pin_order", "INTEGER")?;
    // Earlier versions only stored a boolean. Preserve their displayed order;
    // any legacy pins without a rank follow pins that already have one.
    conn.execute_batch(
        "WITH ranked AS MATERIALIZED (
            SELECT id,
                COALESCE((SELECT MIN(pin_order) FROM clips WHERE is_pinned = 1), COUNT(*) OVER () + 1)
                - ROW_NUMBER() OVER (ORDER BY julianday(last_captured_at) DESC, id ASC) AS rank
            FROM clips WHERE is_pinned = 1 AND pin_order IS NULL
         )
         UPDATE clips SET pin_order = (SELECT rank FROM ranked WHERE ranked.id = clips.id)
         WHERE id IN (SELECT id FROM ranked);

         WITH ranked AS MATERIALIZED (
            SELECT id,
                COALESCE((SELECT MIN(existing.pin_order) FROM category_items existing
                          WHERE existing.category_id = item.category_id AND existing.is_pinned = 1),
                         COUNT(*) OVER (PARTITION BY category_id) + 1)
                - ROW_NUMBER() OVER (PARTITION BY category_id
                                    ORDER BY sort_order ASC, julianday(created_at) DESC, id ASC) AS rank
            FROM category_items item WHERE is_pinned = 1 AND pin_order IS NULL
         )
         UPDATE category_items SET pin_order = (SELECT rank FROM ranked WHERE ranked.id = category_items.id)
         WHERE id IN (SELECT id FROM ranked);

         DROP INDEX IF EXISTS idx_clips_pinned_capture;
         CREATE INDEX IF NOT EXISTS idx_clips_pin_order
         ON clips(is_pinned DESC, pin_order DESC, julianday(last_captured_at) DESC, id ASC);",
    )
    .map_err(|error| error.to_string())
}
