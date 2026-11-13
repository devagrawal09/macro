//! The file name a marketplace copy is uploaded as.

use super::html_file_name;

#[test]
fn a_copy_is_named_after_its_listing_as_an_html_file() {
    assert_eq!(html_file_name("Launch board"), "Launch board.html");
    assert_eq!(html_file_name("  Q3 / Q4 plan\n"), "Q3 - Q4 plan.html");
    assert_eq!(html_file_name("a\\b"), "a-b.html");
}

#[test]
fn a_title_with_nothing_to_keep_gets_a_fallback_name() {
    assert_eq!(html_file_name("   "), "Marketplace copy.html");
    assert_eq!(html_file_name(".."), "Marketplace copy.html");
}
