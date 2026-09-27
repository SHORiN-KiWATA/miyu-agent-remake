use super::*;

#[test]
fn only_the_owner_gets_in() {
    assert_eq!(
        owner_only("S-1-5-21-1-2-3-1001"),
        "D:P(A;;GA;;;S-1-5-21-1-2-3-1001)"
    );
}
