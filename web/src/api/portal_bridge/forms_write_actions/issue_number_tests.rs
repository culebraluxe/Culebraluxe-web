use super::without_issue_number;

#[test]
fn the_issue_number_is_not_part_of_the_subject() {
    assert_eq!(
        without_issue_number("Listing Agreement v9"),
        "Listing Agreement"
    );
    assert_eq!(
        without_issue_number("Listing Agreement v12"),
        "Listing Agreement"
    );
    assert_eq!(
        without_issue_number("Listing Agreement"),
        "Listing Agreement"
    );
    assert_eq!(without_issue_number("Offer v2 draft"), "Offer v2 draft");
}
