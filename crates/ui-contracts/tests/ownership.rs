//! Borrowed event projections cannot escape their sole owner.
#[test]
fn event_query_borrows_the_message() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/event_query_escape.rs");
}
