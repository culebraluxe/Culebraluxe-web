//! PROP.PROPERTY_BASED — pagination (TST-PROP-PROPERTY-BASED-010).
//!
//! Contract: page counts never drop below one (`pages(0, 50) == 1`); the
//! count is the ceiling of `total / size` with a degenerate size treated as
//! one row per page; the list cursor clamps to `[1, pages]` — paging past
//! either end stays put and reports `Nothing` instead of reloading; and a
//! fresh search returns the cursor to page one.
//!
//! Level: L0 Pure — the executable boundary is `ui::app::list`, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__010__pagination

use proptest::prelude::*;
use ui::app::list::{pages, ListChange, ListMsg, ListState};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Counts, clamps and cursor moves hold for every generated total, size
    /// and step; fixed vectors pin the documented forms.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_010__pagination(
        total in 0i64..1_000_000,
        page_size in -100i64..1000,
        step in -200i64..200,
    ) {
        // Fixed positives: the documented counts.
        prop_assert_eq!(pages(0, 50), 1);
        prop_assert_eq!(pages(1, 50), 1);
        prop_assert_eq!(pages(50, 50), 1);
        prop_assert_eq!(pages(51, 50), 2);
        prop_assert_eq!(pages(100, 50), 2);
        prop_assert_eq!(pages(101, 50), 3);
        // Fixed negatives: degenerate sizes and totals still yield one page.
        prop_assert_eq!(pages(10, 0), 10);
        prop_assert_eq!(pages(10, -5), 10);
        prop_assert_eq!(pages(-5, 50), 1);

        // Property: the count is the ceiling of total/size (size floors at
        // 1), and never fewer than one page.
        let size = page_size.max(1);
        let expected = ((total + size - 1) / size).max(1);
        prop_assert_eq!(pages(total, page_size), expected);
        prop_assert!(pages(total, page_size) >= 1);

        // Property: the cursor clamps to [1, pages] — past either end it
        // stays put and reports Nothing; a real move reports Reload.
        let count = pages(total, page_size);
        let mut state = ListState::default();
        prop_assert_eq!(state.page, 1);
        let (change, _) = state.update(ListMsg::Page(step), count);
        let clamped = (1 + step).clamp(1, count.max(1));
        prop_assert_eq!(state.page, clamped, "cursor must clamp into [1, {}]", count);
        prop_assert_eq!(change, if clamped == 1 { ListChange::Nothing } else { ListChange::Reload });

        // Property: from the last page, any forward step stays and is quiet.
        let mut last = ListState::default();
        last.update(ListMsg::Page(count), count);
        prop_assert_eq!(last.page, count);
        let (change, _) = last.update(ListMsg::Page(step.max(0)), count);
        if step.max(0) == 0 {
            prop_assert_eq!(change, ListChange::Nothing);
        } else {
            prop_assert_eq!(last.page, count, "paging past the end must stay");
            prop_assert_eq!(change, ListChange::Nothing);
        }

        // Property: a finished search returns the cursor to page one.
        let mut searching = ListState::default();
        searching.update(ListMsg::Page(count), count);
        let token = {
            let (change, _) = searching.update(ListMsg::Typed("casa".to_string()), count);
            prop_assert_eq!(change, ListChange::Nothing);
            // The pause carries the keystroke token; only the last one counts.
            1u64
        };
        let (change, _) = searching.update(ListMsg::Paused(token), count);
        prop_assert_eq!(change, ListChange::Reload);
        prop_assert_eq!(searching.page, 1);
        // A stale pause token does nothing.
        let (change, _) = searching.update(ListMsg::Paused(token + 1), count);
        prop_assert_eq!(change, ListChange::Nothing);
        prop_assert_eq!(searching.page, 1);
    }
}
