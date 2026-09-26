//! Catches `docs/reference/content-tables.md`'s one remaining hand-copied
//! table — the `DROPS` weights — drifting from the source that actually
//! decides them. Everything else that table used to hardcode (the enchantment
//! odds, the ability chances, the relic's price) now names a constant instead
//! of copying its value, which is what keeps *those* from needing a test like
//! this one: a renamed constant is a compile error, a changed value is simply
//! true wherever it's read from.
//!
//! `DROPS` has no such name to point at — it's a table of category weights,
//! not a single number — so this walks the real `DROPS` array and checks each
//! row's weight is still the one written into the doc's table.

use models::spawn::DROPS;

fn content_tables_md() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/reference/content-tables.md");
    std::fs::read_to_string(path).expect("docs/reference/content-tables.md should exist")
}

/// The `| category | weight | share |` rows of the DROPS table in the doc, as
/// `(name, weight)` pairs. Parsed by splitting on `|` rather than a full
/// markdown parser — this file has exactly one table shaped like this.
fn doc_drop_weights(md: &str) -> Vec<(String, u32)> {
    md.lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(str::trim).collect();
            let [name, weight, _share] = cells.as_slice() else {
                return None;
            };
            let weight: u32 = weight.parse().ok()?;
            // Skip the header's own numeric-looking cells (there are none) and
            // anything that isn't a lowercase category name — cheap enough
            // given the table is nine rows.
            name.chars()
                .all(|c| c.is_ascii_lowercase())
                .then(|| (name.to_string(), weight))
        })
        .collect()
}

#[test]
fn the_drops_table_in_the_doc_matches_the_real_weights() {
    let md = content_tables_md();
    let documented = doc_drop_weights(&md);
    let real: Vec<(String, u32)> = DROPS.iter().map(|c| (c.name.to_string(), c.weight)).collect();

    assert_eq!(
        documented, real,
        "docs/reference/content-tables.md's DROPS table has drifted from \
         `spawn::DROPS` — update the 'Current weights' table (and the share \
         column, which the doc already says is derived, not maintained)"
    );
}
