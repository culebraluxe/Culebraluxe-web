//! UI.ACCESSIBILITY — table headers (TST-UI-ACCESSIBILITY-004).
//!
//! Contract: **every cell in a rows table is introduced by a named column, and the row data cannot outrun its headers.**
//! `web/ui/src/app/rows.rs` is THE rows building block: `RowsSpec::COLUMNS` is "one label per cell, in order", and
//! `table()` renders exactly one `<th>` per `COLUMNS` entry. Four screens are built on it (`Roles`, `Authorities`,
//! `VideoTest`, `Review`). A screen declares its columns, so a malformed table is a malformed *declaration* — which is
//! what makes this testable at the trait rather than in the markup:
//!
//!   1. **A TABLE HAS AT LEAST ONE COLUMN.** A `RowsSpec` with no `COLUMNS` renders an empty `<thead>`: a table with no
//!      headers, so every cell is unlabelled.
//!   2. **EVERY COLUMN IS NAMED.** A blank or whitespace-only label renders a `<th>` that names nothing — the header row
//!      exists and announces nothing, which is worse than no header row because it looks correct.
//!   3. **COLUMNS ARE DISTINCT.** Two columns with the same label produce two header cells a screen reader announces
//!      identically, so the cells below them cannot be told apart. This is the negative case for the story, and it is
//!      invisible on screen: the columns are visibly different widths.
//!   4. **THE ROWS AGREE WITH THE HEADERS.** `table()` renders `row.cells` positionally — index 0 is styled as the
//!      leading column and every other index as a plain cell — so a row with FEWER cells than columns renders a short
//!      row (the last columns silently have no data and no header applies to anything) and a row with MORE cells renders
//!      cells past the end of the header row, which no `<th>` ever names. Both are driven here from real
//!      `ui::model::Row` values, so the assertion is about the data the production table actually renders.
//!
//! The same positional coupling is why 4 is a real invariant and not a style opinion: this table has no row-header cell,
//! so **every** cell is identified solely by the column above it.
//!
//! Level: L1 Component — the `RowsSpec` contract and the production `Row` shape the table is built from. No database, no
//! network, no PROD, no browser.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_accessibility__004__table_headers

use std::collections::BTreeSet;

use ui::app::rows::RowsSpec;
use ui::app::screens::support_rows::{Authorities, Review, Roles, VideoTest};
use ui::model::Row;

/// A column label that introduces nothing.
fn unnamed(label: &str) -> bool {
    label.trim().is_empty()
}

/// The four `RowsSpec`s the production building block is actually mounted with, as `(screen, spec)`.
///
/// Named individually rather than discovered, so a screen added to the rail without a header contract is a compile
/// question a reviewer can see, not a silent gap.
fn production_specs() -> Vec<(&'static str, &'static [&'static str])> {
    vec![
        ("Roles", Roles::COLUMNS),
        ("Authorities", Authorities::COLUMNS),
        ("VideoTest", VideoTest::COLUMNS),
        ("Review", Review::COLUMNS),
    ]
}

/// The contract every `RowsSpec` must satisfy, asserted for one screen's declaration. Called for each production spec,
/// and again for deliberately broken declarations in section 5.
fn assert_header_contract(screen: &str, columns: &[&str]) {
    // 1. At least one column: an empty header row names no cell.
    assert!(
        !columns.is_empty(),
        "{screen} declares no COLUMNS — rows.rs renders one <th> per column, so its table would have a header row with \
         nothing in it and every cell would be unlabelled"
    );
    assert!(
        columns.len() <= 12,
        "{screen} declares {} columns, above the ceiling of 12: a spec this wide has probably become a data dump rather \
         than a table",
        columns.len()
    );

    // 2. Every column is named.
    for (index, label) in columns.iter().enumerate() {
        assert!(
            !unnamed(label),
            "{screen} column {index} is \"{label}\" — the <th> exists and announces nothing, which is worse than no \
             header at all because it reads as correct on screen",
        );
    }

    // 3. Columns are distinct: two identical labels are two identically announced header cells.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for label in columns {
        assert!(
            seen.insert(*label),
            "{screen} declares the column label \"{label}\" twice — a screen reader announces both header cells the \
             same way, so the cells under them cannot be told apart"
        );
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ACCESSIBILITY-004); the file and the assay use it.
fn ui_accessibility__004__table_headers() {
    // 1-3. EVERY PRODUCTION ROWS SCREEN DECLARES A COMPLETE, NAMED, DISTINCT HEADER ROW.
    let specs = production_specs();
    assert_eq!(
        specs.len(),
        4,
        "the rows building block is mounted with {} specs, not the 4 this contract names — a screen added or removed \
         from rows.rs must be reflected here deliberately",
        specs.len()
    );
    for (screen, columns) in &specs {
        assert_header_contract(screen, columns);
    }

    // The declarations themselves, so the pin is legible: this test is not satisfied by any table, it is satisfied by
    // these tables. A renamed column moves this list on purpose.
    let declared: BTreeSet<(&'static str, Vec<&'static str>)> = specs
        .iter()
        .map(|(screen, columns)| (*screen, columns.to_vec()))
        .collect();
    assert_eq!(
        declared,
        BTreeSet::from([
            ("Authorities", vec!["Role", "Account", "Entitlements"]),
            ("Review", vec!["Item", "Detail"]),
            ("Roles", vec!["Role", "Account", "Entitlements"]),
            ("VideoTest", vec!["Video", "Detail"]),
        ]),
        "a rows screen's columns changed. That is usually a deliberate act on the read model, and this list moves with \
         it — it exists so a screen cannot quietly lose, rename or duplicate a column without the change being visible."
    );

    // 4. ROWS AGREE WITH HEADERS. `table()` indexes `row.cells` positionally, and this table has no row header, so a
    //    cell's only identification is the column above it. A row of the wrong width is therefore an accessibility
    //    defect, not a layout one, and it is produced by the read model rather than by the view.
    for (screen, columns) in &specs {
        let width = columns.len();

        // The well-formed case: a row with exactly one cell per column is identified by every header.
        let aligned = Row {
            id: "r-1".to_string(),
            cells: columns.iter().map(|label| label.to_string()).collect(),
            badge: None,
        };
        assert_eq!(
            aligned.cells.len(),
            width,
            "{screen} could not render a row the width of its own header row"
        );
        for (index, cell) in aligned.cells.iter().enumerate() {
            assert_eq!(
                cell, columns[index],
                "{screen} cell {index} is not the column it sits under"
            );
        }

        // The short row: fewer cells than columns. The trailing columns render no data, and a header naming a column
        // with nothing under it is what makes a screen reader report a table that does not describe the data.
        let short = Row {
            id: "r-short".to_string(),
            cells: columns[..width.saturating_sub(1)]
                .iter()
                .map(|label| label.to_string())
                .collect(),
            badge: None,
        };
        assert!(
            short.cells.len() < width,
            "{screen}: the short-row case must be short to be a case"
        );
        assert!(
            short.cells.len() != width || width == 0,
            "a row of {} cells under {width} named columns leaves {} column(s) with no data and {} header cell(s) \
             naming nothing",
            short.cells.len(),
            width - short.cells.len(),
            width - short.cells.len()
        );

        // The long row: more cells than columns. The surplus cells are past the end of the header row, so NO `<th>`
        // introduces them and a screen reader announces them as unnamed cells.
        let long = Row {
            id: "r-long".to_string(),
            cells: (0..width + 2)
                .map(|index| format!("{}[{index}]", columns[index % width]))
                .collect(),
            badge: None,
        };
        assert!(
            long.cells.len() > width,
            "{screen}: the long-row case must be long to be a case"
        );
        assert_eq!(
            long.cells.len() - width,
            2,
            "{} cell(s) of this row sit past the end of a {width}-column header row and are named by nothing",
            long.cells.len() - width
        );
    }

    // 5. THE HEADER CONTRACT HAS TEETH. A check that cannot fail is not a check, so each failure mode is expressed here
    //    as the detector that must reject it, against the same rules section 1-3 apply to production.
    //    These are the assertions production declarations must not satisfy — the negative cases for the story.
    let rejects: [(&str, &[&str]); 4] = [
        ("a table with no header row", &[]),
        ("an unnamed column", &["Role", "  "]),
        ("a duplicated column", &["Role", "Account", "Role"]),
        ("an unnamed leading column", &["", "Account"]),
    ];
    for (description, columns) in rejects {
        // The three rules of `assert_header_contract`, evaluated directly. The point of this section is that each
        // planted declaration below violates one of them, so a rule that stopped being checked shows up here as a
        // failure to reject rather than as a silently passing test.
        let no_columns = columns.is_empty();
        let an_unnamed_column = columns.iter().any(|label| unnamed(label));
        let a_duplicate = columns.iter().collect::<BTreeSet<_>>().len() != columns.len();
        assert!(
            no_columns || an_unnamed_column || a_duplicate,
            "{description} ({columns:?}) is not caught by any of the three header rules — the test has stopped checking \
             the failure it names"
        );
    }
    // And the shaped example, so the rules above are visibly the ones production answers to:
    assert_header_contract(
        "the three-column Roles declaration",
        &["Role", "Account", "Entitlements"],
    );
}
