use super::*;

#[test]
fn prices_and_areas_read_as_the_typescript_cards_did() {
    assert_eq!(format_price(Some(2_350_000.0)), "$2,350,000");
    assert_eq!(format_price(Some(0.0)), "Price Upon Request");
    assert_eq!(format_price(None), "Price Upon Request");
    assert_eq!(
        format_area(Some(1.0), Some("Acres")).as_deref(),
        Some("1 Acre")
    );
    assert_eq!(
        format_area(Some(2.5), Some("acres")).as_deref(),
        Some("2.5 Acres")
    );
    assert_eq!(
        format_area(Some(12500.0), Some("SqFt")).as_deref(),
        Some("12,500 SF")
    );
    assert_eq!(en_us(1234.5), "1,234.5");
    assert_eq!(whole(&json!(2224613.0)), json!(2224613));
    assert_eq!(whole(&json!(1.5)), json!(1.5));
}
