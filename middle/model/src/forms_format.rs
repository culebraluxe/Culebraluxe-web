//! Field formatting for forms — the same rules the composer and the editor share.
//!
//! PORTED FROM `lib/forms/format.ts`, NUMBER FOR NUMBER, and moved here from the server's composer so the WASM editor can
//! use them too: a money field must read the same in the input as it does in the PDF, and two implementations of "format
//! money" would drift on the first edge case. Pure functions, no dependencies.

use crate::forms_template::{TemplateFieldDefinition, TemplateFieldType};

/// Deterministic USD formatting for money fields.
pub fn format_money(value: &str) -> String {
    let digits: String = value
        .chars()
        .filter(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    if digits.is_empty() {
        return value.trim().to_string();
    }
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (digits.as_str(), None),
    };
    let mut grouped = String::with_capacity(whole.len() + whole.len() / 3);
    for (index, character) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    match fraction {
        Some(fraction) => format!("${grouped}.{fraction}"),
        None => format!("${grouped}"),
    }
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
    fn money_groups_thousands_and_keeps_the_typed_decimal() {
        assert_eq!(format_money("1250000"), "$1,250,000");
        assert_eq!(format_money("1250000.5"), "$1,250,000.5");
        assert_eq!(format_money("$1,250,000"), "$1,250,000");
        assert_eq!(format_money("not a number"), "not a number");
        assert_eq!(format_money(""), "");
    }

    #[test]
    fn dates_render_as_prose_and_anything_else_passes_through() {
        assert_eq!(format_date("2026-01-05"), "January 5, 2026");
        assert_eq!(format_date("2026-13-05"), "2026-13-05");
        assert_eq!(format_date("next week"), "next week");
    }
}
