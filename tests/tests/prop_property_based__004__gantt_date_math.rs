//! PROP.PROPERTY_BASED — Gantt date math (TST-PROP-PROPERTY-BASED-004).
//!
//! Contract: planned spans are inclusive calendar days
//! (`duration = finish - start + 1`, a single day is 1); a finish-to-start
//! link is broken exactly when the dependent starts on or before the day its
//! predecessor finishes; unscheduled work breaks nothing; planned dates are
//! strict `YYYY-MM-DD` with start never after finish.
//!
//! Level: L0 Pure — the executable boundaries are `model::wbs` and
//! `ui::timeline`, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__004__gantt_date_math

use chrono::NaiveDate;
use model::wbs::validate_planned_dates;
use proptest::prelude::*;
use ui::timeline::{date, link_broken, planned_duration_days};

/// Always-valid calendar days (day capped at 28, so every month qualifies).
fn calendar_day() -> impl Strategy<Value = NaiveDate> {
    (2020i32..2031, 1u32..13, 1u32..29).prop_map(|(year, month, day)| {
        NaiveDate::from_ymd_opt(year, month, day).expect("day <= 28 is always valid")
    })
}

fn date_string() -> impl Strategy<Value = String> {
    calendar_day().prop_map(|d| d.format("%Y-%m-%d").to_string())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Inclusive durations, the broken-link rule, and strict date validation
    /// hold for every generated span; fixed vectors pin the documented forms.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_004__gantt_date_math(
        start in calendar_day(),
        finish in calendar_day(),
        raw in date_string(),
    ) {
        // Fixed positives: the documented forms.
        let from = date("2026-09-10").unwrap();
        let to = date("2026-09-12").unwrap();
        prop_assert_eq!(planned_duration_days(from, to), 3);
        prop_assert_eq!(planned_duration_days(from, from), 1);
        prop_assert!(validate_planned_dates(Some("2026-09-10"), Some("2026-09-12")).is_ok());
        prop_assert!(validate_planned_dates(Some("2026-09-10"), Some("2026-09-10")).is_ok());
        prop_assert!(validate_planned_dates(None, None).is_ok());
        prop_assert!(validate_planned_dates(Some("2026-09-10"), None).is_ok());
        prop_assert!(validate_planned_dates(None, Some("2026-09-12")).is_ok());
        // Fixed negatives: reversed spans, impossible dates, datetimes.
        prop_assert!(validate_planned_dates(Some("2026-09-12"), Some("2026-09-10")).is_err());
        prop_assert!(validate_planned_dates(Some("2026-02-30"), None).is_err());
        prop_assert!(validate_planned_dates(Some("2026-09-10T00:00:00Z"), None).is_err());
        // Fixed link rule: start on-or-before finish is broken; gaps are fine.
        prop_assert!(link_broken(date("2026-09-12"), date("2026-09-12")));
        prop_assert!(link_broken(date("2026-09-12"), date("2026-09-10")));
        prop_assert!(!link_broken(date("2026-09-10"), date("2026-09-12")));
        prop_assert!(!link_broken(None, date("2026-09-12")));
        prop_assert!(!link_broken(date("2026-09-10"), None));

        // Property: duration is the inclusive day count, and a point span is 1.
        prop_assert_eq!(
            planned_duration_days(start, finish),
            (finish - start).num_days() + 1
        );
        prop_assert_eq!(planned_duration_days(start, start), 1);

        // Property: the broken-link rule is exactly `start <= finish`, and
        // unscheduled work breaks nothing.
        prop_assert_eq!(
            link_broken(Some(finish), Some(start)),
            start <= finish,
            "link {} -> {}",
            finish,
            start
        );
        prop_assert!(!link_broken(None, Some(start)));
        prop_assert!(!link_broken(Some(finish), None));
        prop_assert!(!link_broken(None, None));

        // Property: a generated `YYYY-MM-DD` string parses, and validation
        // accepts it on either side alone.
        prop_assert!(date(&raw).is_some(), "generated date must parse: {}", raw);
        prop_assert!(validate_planned_dates(Some(&raw), None).is_ok());
        prop_assert!(validate_planned_dates(None, Some(&raw)).is_ok());
        // Property: validation of a full span agrees with calendar order.
        let (early, late) = if start <= finish { (start, finish) } else { (finish, start) };
        let (early_s, late_s) = (
            early.format("%Y-%m-%d").to_string(),
            late.format("%Y-%m-%d").to_string(),
        );
        prop_assert!(validate_planned_dates(Some(&early_s), Some(&late_s)).is_ok());
        if early != late {
            prop_assert!(validate_planned_dates(Some(&late_s), Some(&early_s)).is_err());
        }
    }
}
