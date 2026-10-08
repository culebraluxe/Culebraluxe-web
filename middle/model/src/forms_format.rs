//! Field formatting for forms — the same rules the composer and the editor share.
//!
//! PORTED FROM `lib/forms/format.ts`, NUMBER FOR NUMBER, and moved here from the server's composer so the WASM editor can
//! use them too: a money field must read the same in the input as it does in the PDF, and two implementations of "format
//! money" would drift on the first edge case. Pure functions, no dependencies.

use crate::forms_template::{TemplateFieldDefinition, TemplateFieldType};

/// Deterministic USD formatting for money fields: `$X,XXX,XXX.XX`, the way a spreadsheet writes money. Always two
/// decimals, rounded to the cent (half up); a leading minus is kept (`-$1,250.00`); anything with no digits in it is
/// returned as typed rather than turned into a number it never was.
pub fn format_money(value: &str) -> String {
    let trimmed = value.trim();
    let negative = trimmed.starts_with('-');
    let numeric: String = trimmed
        .chars()
        .filter(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    if !numeric.chars().any(|character| character.is_ascii_digit()) {
        return trimmed.to_string();
    }
    let (whole, fraction) = numeric.split_once('.').unwrap_or((numeric.as_str(), ""));
    let fraction: String = fraction.chars().filter(char::is_ascii_digit).collect();
    // Whole dollars and cents as integers: the cents come from the first two fraction digits, and the third decides
    // the rounding. u128 holds 38 digits, which no price reaches; a longer run is passed through, not mangled.
    let Ok(dollars) = (if whole.is_empty() { "0" } else { whole }).parse::<u128>() else {
        return trimmed.to_string();
    };
    let mut digits = fraction
        .chars()
        .map(|character| character.to_digit(10).unwrap_or(0));
    let tens = digits.next().unwrap_or(0);
    let ones = digits.next().unwrap_or(0);
    let round_up = digits.next().is_some_and(|digit| digit >= 5);
    let mut cents = u128::from(tens * 10 + ones) + u128::from(round_up);
    let mut dollars = dollars;
    if cents >= 100 {
        cents -= 100;
        dollars += 1;
    }
    let whole = dollars.to_string();
    let mut grouped = String::with_capacity(whole.len() + whole.len() / 3);
    for (index, character) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    let sign = if negative && (dollars > 0 || cents > 0) {
        "-"
    } else {
        ""
    };
    format!("{sign}${grouped}.{cents:02}")
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Deterministic ISO (`YYYY-MM-DD`) to `Month D, YYYY`. A value of any other shape passes through unchanged.
pub fn format_date(value: &str) -> String {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return trimmed.to_string();
    }
    let Ok(year) = trimmed[0..4].parse::<i32>() else {
        return trimmed.to_string();
    };
    let Ok(month) = trimmed[5..7].parse::<usize>() else {
        return trimmed.to_string();
    };
    let Ok(day) = trimmed[8..10].parse::<u32>() else {
        return trimmed.to_string();
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return trimmed.to_string();
    }
    format!("{} {day}, {year}", MONTHS[month - 1])
}

/// Format one field value for display and for rendering: money and dates are formatted, everything else is left alone.
pub fn format_field_value(field: &TemplateFieldDefinition, raw: &str) -> String {
    match field.field_type {
        TemplateFieldType::Money if !raw.trim().is_empty() => format_money(raw),
        TemplateFieldType::Date if !raw.trim().is_empty() => format_date(raw),
        _ => raw.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_is_dollars_and_cents_grouped_in_thousands() {
        assert_eq!(format_money("1250000"), "$1,250,000.00");
        assert_eq!(format_money("1250000.5"), "$1,250,000.50");
        assert_eq!(format_money("$1,250,000"), "$1,250,000.00");
        assert_eq!(format_money("not a number"), "not a number");
        assert_eq!(format_money(""), "");
    }

    #[test]
    fn dates_render_as_prose_and_anything_else_passes_through() {
        assert_eq!(format_date("2026-01-05"), "January 5, 2026");
        assert_eq!(format_date("2026-13-05"), "2026-13-05");
        assert_eq!(format_date("next week"), "next week");
    }

    #[test]
    fn money_reads_like_a_spreadsheet_cell() {
        assert_eq!(format_money("5000000"), "$5,000,000.00");
        assert_eq!(
            format_money("999.999"),
            "$1,000.00",
            "rounds to the cent and carries"
        );
        assert_eq!(format_money("0.005"), "$0.01");
        assert_eq!(format_money("0.004"), "$0.00");
        assert_eq!(format_money(".5"), "$0.50");
        assert_eq!(format_money("1250000."), "$1,250,000.00");
        assert_eq!(format_money("-1250.5"), "-$1,250.50");
        assert_eq!(format_money("  $ 12 345.60 "), "$12,345.60");
        assert_eq!(
            format_money("1,2,3.4.5"),
            "$123.45",
            "stray separators do not break it"
        );
        assert_eq!(format_money("-0"), "$0.00", "no negative zero");
        assert_eq!(format_money("TBD"), "TBD");
    }
}
