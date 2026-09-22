//! The display forms the portal's accounting screens are covered in — money and dates.
//!
//! WHY THIS IS NOT INSIDE THE VIEW MODULE. The view module is compiled only for the browser (`wasm` + `yew`), and these
//! functions are pure: a string in, a string out, with no DOM, no state and no effect. Kept here they run in
//! `cargo test -p ui` on an ordinary host target, which is the only way the rounding rules below can be proved at all. A
//! rounding bug in a money label is invisible in review and obvious on a reconciliation.
//!
//! MONEY IS FORMATTED FROM ITS DIGITS, NEVER FROM A FLOAT. The live screen used
//! `Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 0 })`: whole dollars, grouped
//! thousands. Reproducing that by parsing the decimal into an `f64` would defeat the reason the amount is a string in the
//! first place — a large enough total loses its cents and then its dollars to binary rounding. The formatter below rounds
//! and groups the DIGITS, with integer arithmetic, and never leaves the decimal world.

/// The month abbreviations a date label is built from, as the locale the live screen formatted in produced them.
const MONTH_LABELS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Round a decimal string to whole dollars and group it: `1250.50` becomes `$1,251`.
///
/// Rounding is half-away-from-zero with the sign in front of the symbol (`-$1,251`), which is what `Intl` does for
/// currency. A value that rounds to zero prints without a sign, so a micropayment is not shown as a negative nothing.
pub fn format_money(amount: &str) -> String {
    let trimmed = amount.trim();
    let negative = trimmed.starts_with('-');
    let digits = trimmed.strip_prefix(['-', '+']).unwrap_or(trimmed);
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (digits, ""),
    };
    // Grouping happens after rounding, so a carry that crosses a group boundary is handled once rather than twice.
    let mut whole_digits = whole
        .chars()
        .filter_map(|character| character.to_digit(10))
        .collect::<Vec<u32>>();
    if whole_digits.is_empty() {
        whole_digits.push(0);
    }
    // Only the leading fraction digit decides the direction: every later one is smaller than the half, so a `4999` tail
    // against a `.5` lead cannot change the answer.
    let round_up = fraction
        .chars()
        .next()
        .and_then(|character| character.to_digit(10))
        .is_some_and(|digit| digit >= 5);
    if round_up {
        let mut index = whole_digits.len();
        loop {
            if index == 0 {
                whole_digits.insert(0, 1);
                break;
            }
            index -= 1;
            if whole_digits[index] == 9 {
                whole_digits[index] = 0;
            } else {
                whole_digits[index] += 1;
                break;
            }
        }
    }
    let zero = whole_digits.iter().all(|digit| *digit == 0);
    let length = whole_digits.len();
    let grouped = whole_digits
        .iter()
        .enumerate()
        .fold(String::new(), |mut rendered, (index, digit)| {
            // A separator before every third digit counted from the right, and never leading.
            if index > 0 && (length - index) % 3 == 0 {
                rendered.push(',');
            }
            rendered.push(char::from_digit(*digit, 10).unwrap_or('0'));
            rendered
        });
    if negative && !zero {
        format!("-${grouped}")
    } else {
        format!("${grouped}")
    }
}

/// `2026-08-27` becomes `Aug 27, 2026`, which is the label the live screen printed.
///
/// PARSED FROM THE SHAPE, NOT BY A DATE LIBRARY. The value is the `YYYY-MM-DD` a Postgres `date` column renders, and a
/// calendar dependency in the browser bundle — for a label with no timezone, no arithmetic and no locale — would be weight
/// for nothing. What is validated is exactly what the label needs: three numeric parts in range.
///
/// A value that is not that shape is passed through rather than blanked: an empty cell hides a data problem, and a visible
/// oddity reports one. An absent value prints as nothing, because "no due date" is a real state the caller renders as its
/// own dash.
pub fn format_date(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let mut parts = value.split('-');
    let parsed = (|| {
        let year = parts.next()?.parse::<i32>().ok()?;
        let month = parts.next()?.parse::<usize>().ok()?;
        let day = parts.next()?.parse::<u32>().ok()?;
        if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        Some((year, month, day))
    })();
    match parsed {
        Some((year, month, day)) => format!("{} {}, {}", MONTH_LABELS[month - 1], day, year),
        None => value.to_owned(),
    }
}

/// Whether an amount is zero, decided from the digits rather than by parsing a float.
///
/// Used to choose between a chart and its empty state: a month whose expenses all cancelled to nothing should say so
/// instead of drawing a ring with no slices.
pub fn is_zero(amount: &str) -> bool {
    amount
        .trim()
        .chars()
        .all(|character| matches!(character, '0' | '.' | '-' | '+'))
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_is_rounded_to_whole_dollars_the_way_intl_rounded_it() {
        assert_eq!(format_money("1250.50"), "$1,251");
        assert_eq!(format_money("1250.49"), "$1,250");
        assert_eq!(format_money("0.00"), "$0");
        assert_eq!(format_money("12000"), "$12,000");
        // A carry through a group boundary: every digit behind it is a nine.
        assert_eq!(format_money("999999.99"), "$1,000,000");
        assert_eq!(format_money(" 42.5 "), "$43");
    }

    #[test]
    fn a_negative_amount_keeps_its_sign_in_front_of_the_symbol() {
        assert_eq!(format_money("-1250.50"), "-$1,251");
        // A value that rounds to zero IS zero, and a negative nothing would be misleading.
        assert_eq!(format_money("-0.40"), "$0");
    }

    #[test]
    fn money_never_travels_through_a_float() {
        // The whole reason these are strings: `f64` cannot hold this amount, so a formatter that parsed it would print a
        // different number than the one the database holds.
        assert_eq!(
            format_money("123456789012345678.90"),
            "$123,456,789,012,345,679"
        );
    }

    #[test]
    fn a_value_that_is_not_a_number_prints_as_zero_rather_than_panicking() {
        // A screen must not come down over a label. `$0` here is visible and wrong; a panic is invisible and total.
        assert_eq!(format_money(""), "$0");
        assert_eq!(format_money("n/a"), "$0");
    }

    #[test]
    fn dates_are_labelled_the_way_the_live_screen_labelled_them() {
        assert_eq!(format_date(Some("2026-08-27")), "Aug 27, 2026");
        assert_eq!(format_date(Some("2026-01-01")), "Jan 1, 2026");
        assert_eq!(format_date(Some("2026-12-31")), "Dec 31, 2026");
    }

    #[test]
    fn an_absent_or_odd_date_is_handled_without_inventing_one() {
        assert_eq!(format_date(None), "");
        assert_eq!(format_date(Some("   ")), "");
        // Passed through rather than blanked: a visible oddity reports the data problem an empty cell would hide.
        assert_eq!(format_date(Some("2026-13-40")), "2026-13-40");
        assert_eq!(format_date(Some("August")), "August");
        assert_eq!(
            format_date(Some("2026-08-27T10:00:00Z")),
            "2026-08-27T10:00:00Z"
        );
    }

    #[test]
    fn zero_is_decided_from_the_digits() {
        assert!(is_zero("0.00"));
        assert!(is_zero("0"));
        assert!(is_zero("-0.0"));
        assert!(!is_zero("0.01"));
        assert!(!is_zero("1250.00"));
    }
}
