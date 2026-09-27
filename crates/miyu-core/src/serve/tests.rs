use super::*;

#[test]
fn it_looks_a_quarter_of_the_idle_time_apart_within_bounds() {
    assert_eq!(check(Duration::from_secs(600)), Duration::from_secs(30));
    assert_eq!(check(Duration::from_secs(60)), Duration::from_secs(15));
    assert_eq!(
        check(Duration::from_millis(200)),
        Duration::from_millis(100)
    );
}
