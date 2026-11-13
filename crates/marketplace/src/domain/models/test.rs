//! Reading a page request.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{ListingCursor, ListingId, ListingQuery, ListingsShown, MarketplaceError};

#[test]
fn a_cursor_reads_back_as_the_listing_it_names() {
    let cursor = ListingCursor {
        created_at: DateTime::<Utc>::from_timestamp_micros(1_793_491_200_123_456)
            .expect("a valid instant"),
        id: ListingId::from_uuid(Uuid::from_u128(7)),
    };

    let read = ListingCursor::decode(&cursor.encode());

    assert_eq!(read.ok(), Some(cursor));
}

#[test]
fn a_cursor_that_is_not_one_is_refused() {
    for cursor in [
        "",
        "page-2",
        "12.not-a-uuid",
        "x.00000000-0000-0000-0000-000000000007",
    ] {
        let read = ListingCursor::decode(cursor);

        assert!(
            matches!(read, Err(MarketplaceError::Invalid(_))),
            "{cursor:?} was read"
        );
    }
}

#[test]
fn a_page_holds_fifty_unless_asked_for_one_to_a_hundred() {
    let page = |limit| ListingQuery::new(ListingsShown::Listed, None, None, None, None, limit);

    assert_eq!(page(None).map(|query| query.limit).ok(), Some(50));
    assert_eq!(page(Some(1)).map(|query| query.limit).ok(), Some(1));
    assert_eq!(page(Some(100)).map(|query| query.limit).ok(), Some(100));
    assert!(matches!(page(Some(0)), Err(MarketplaceError::Invalid(_))));
    assert!(matches!(page(Some(101)), Err(MarketplaceError::Invalid(_))));
}

#[test]
fn a_blank_search_searches_nothing_and_a_long_one_is_refused() {
    let search =
        |text: &str| ListingQuery::new(ListingsShown::Listed, Some(text), None, None, None, None);

    assert_eq!(search("  ").map(|query| query.search).ok(), Some(None));
    assert_eq!(
        search(" board ").map(|query| query.search).ok(),
        Some(Some("board".to_owned()))
    );
    assert!(search(&"é".repeat(200)).is_ok());
    assert!(matches!(
        search(&"é".repeat(201)),
        Err(MarketplaceError::Invalid(_))
    ));
}
