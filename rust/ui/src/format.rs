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

use crate::model::Listing;

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

// ---- Listing cards ----------------------------------------------------------------------------------------------------
//
// ONE PLACE FOR WHAT A CARD SAYS. The homepage, the portfolio band and the buyers inventory each built their own facts
// line and their own missing-price wording, and they drifted: "8 Bed" beside "8 Beds", "Price upon request" beside
// "Price on request". These are the single answers, and they are pure so `cargo test -p ui` can pin them.

/// What a card shows when a listing has no price. Matches `formatPrice` in `lib/property.ts` and the detail page.
pub const PRICE_ON_REQUEST: &str = "Price Upon Request";

/// How much a facts line may say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactsStyle {
    /// The small portfolio card: beds and baths (or the lot, for land), then the leading view. This is the line the
    /// TypeScript card drew, restored.
    Compact,
    /// The full-width cards: beds, baths, interior and lot — or the lot alone, for land.
    Full,
}

/// Whether a listing is land, by its type. The card's Land badge, the facts line and the buyers tabs all ask this.
pub fn listing_is_land(listing: &Listing) -> bool {
    listing
        .kind
        .as_deref()
        .is_some_and(|kind| kind.to_ascii_lowercase().contains("land"))
}

/// A count as a person writes it: "8", not "8.0", while a half-bath keeps its half ("7.5").
fn count(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn present(value: &Option<String>) -> Option<String> {
    value.as_deref().map(str::trim).filter(|value| !value.is_empty()).map(str::to_string)
}

/// The one-line facts under a listing's name, joined with a spaced middle dot. Missing numbers are left out, never
/// printed as zero.
pub fn listing_facts(listing: &Listing, style: FactsStyle) -> String {
    listing_fact_parts(listing, style).join("  \u{00b7}  ")
}

/// The same facts, one per entry, for a view that sets its own separators (HTML collapses the spaced dot to a squeeze).
pub fn listing_fact_parts(listing: &Listing, style: FactsStyle) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    if listing_is_land(listing) {
        parts.extend(present(&listing.area));
    } else {
        if let Some(beds) = listing.beds {
            parts.push(format!("{} Bed", count(beds)));
        }
        if let Some(baths) = listing.baths {
            parts.push(format!("{} Bath", count(baths)));
        }
        if style == FactsStyle::Full {
            parts.extend(present(&listing.interior_area));
            parts.extend(present(&listing.area));
        }
    }
    if style == FactsStyle::Compact {
        if let Some(view) = listing.views.iter().find(|view| !view.trim().is_empty()) {
            parts.push(format!("{} View", view.trim()));
        }
    }
    parts
}

/// A listing's price for display, or the request wording when it has none.
pub fn listing_price_label(listing: &Listing) -> String {
    present(&listing.price).unwrap_or_else(|| PRICE_ON_REQUEST.to_string())
}

/// The small line above a listing's name: its type and its place, whichever it has ("Luxury Estate · Zoni, Culebra").
pub fn listing_eyebrow(listing: &Listing) -> Option<String> {
    let parts = [present(&listing.kind), present(&listing.location)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("  \u{00b7}  "))
}

/// Up to `limit` short highlights for a card, from the listing's own flags: its leading view, then beach access, then
/// its next view. Nothing here is written for one listing — a property with no views and no access gets no highlights.
pub fn listing_highlights(listing: &Listing, limit: usize) -> Vec<String> {
    let mut views = listing
        .views
        .iter()
        .map(|view| view.trim())
        .filter(|view| !view.is_empty());
    let mut highlights: Vec<String> = Vec::new();
    highlights.extend(views.next().map(|view| format!("{view} View")));
    if listing.beach_access {
        highlights.push("Beach Access".to_string());
    }
    highlights.extend(views.map(|view| format!("{view} View")));
    highlights.truncate(limit);
    highlights
}

/// Where "Enquire" goes: the contact form, told which property and that this is a viewing request — the same link the
/// detail page's "Request a private viewing" builds. Without an id it is still the contact form, never a dead anchor.
pub fn listing_enquire_href(listing: &Listing) -> String {
    if listing.id.trim().is_empty() {
        "/contact?requestType=private_viewing#contact".to_string()
    } else {
        format!(
            "/contact?propertyId={}&requestType=private_viewing#contact",
            listing.id.trim()
        )
    }
}

#[cfg(test)]
mod listing_tests {
    use super::*;

    fn residence() -> Listing {
        Listing {
            id: "p-1".into(),
            slug: "estate".into(),
            name: "Estate".into(),
            location: Some("Zoni, Culebra".into()),
            price: Some("$2,500,000".into()),
            kind: Some("Luxury Estate".into()),
            beds: Some(8.0),
            baths: Some(7.5),
            area: Some("1 Acre".into()),
            interior_area: Some("6,399 SF".into()),
            views: vec!["Ocean".into(), "Beach".into(), "Island".into()],
            beach_access: true,
            ..Listing::default()
        }
    }

    #[test]
    fn a_residence_leads_with_its_interior_and_keeps_its_lot() {
        assert_eq!(
            listing_facts(&residence(), FactsStyle::Full),
            "8 Bed  \u{b7}  7.5 Bath  \u{b7}  6,399 SF  \u{b7}  1 Acre"
        );
    }

    #[test]
    fn the_compact_line_is_the_one_the_typescript_card_drew() {
        assert_eq!(
            listing_facts(&residence(), FactsStyle::Compact),
            "8 Bed  \u{b7}  7.5 Bath  \u{b7}  Ocean View"
        );
    }

    #[test]
    fn land_is_described_by_its_lot_alone() {
        let land = Listing {
            kind: Some("Land".into()),
            beds: Some(0.0),
            area: Some("2.3 Acres".into()),
            views: vec![],
            ..residence()
        };
        assert_eq!(listing_facts(&land, FactsStyle::Full), "2.3 Acres");
        assert_eq!(listing_facts(&land, FactsStyle::Compact), "2.3 Acres");
    }

    #[test]
    fn missing_facts_are_left_out_not_printed_as_zero() {
        let sparse = Listing::default();
        assert_eq!(listing_facts(&sparse, FactsStyle::Full), "");
        assert_eq!(listing_eyebrow(&sparse), None);
    }

    #[test]
    fn every_card_says_the_same_thing_without_a_price() {
        let unpriced = Listing { price: Some("  ".into()), ..residence() };
        assert_eq!(listing_price_label(&unpriced), PRICE_ON_REQUEST);
        assert_eq!(listing_price_label(&residence()), "$2,500,000");
    }

    #[test]
    fn highlights_come_from_the_listing_flags_in_a_fixed_order() {
        assert_eq!(listing_highlights(&residence(), 2), vec!["Ocean View", "Beach Access"]);
        let inland = Listing { views: vec![], beach_access: false, ..residence() };
        assert!(listing_highlights(&inland, 2).is_empty());
    }

    #[test]
    fn enquire_goes_to_the_contact_form_never_a_dead_anchor() {
        assert_eq!(
            listing_enquire_href(&residence()),
            "/contact?propertyId=p-1&requestType=private_viewing#contact"
        );
        assert!(listing_enquire_href(&Listing::default()).starts_with("/contact?"));
    }

    #[test]
    fn the_eyebrow_joins_type_and_place() {
        assert_eq!(
            listing_eyebrow(&residence()).as_deref(),
            Some("Luxury Estate  \u{b7}  Zoni, Culebra")
        );
    }
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
