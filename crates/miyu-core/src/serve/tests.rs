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

/// 装不上的信号当它不会来（施工 4-9 再补三上）：一直不回；装上了的照常回。
#[tokio::test]
async fn a_signal_that_could_not_be_watched_never_arrives() {
    let failed = watched(async { Err(std::io::Error::other("not installed")) });
    let waited = tokio::time::timeout(std::time::Duration::from_millis(50), failed).await;
    assert!(waited.is_err(), "装不上的不当成收到了");
    let arrived = watched(async { Ok(()) });
    let waited = tokio::time::timeout(std::time::Duration::from_secs(5), arrived).await;
    assert!(waited.is_ok(), "装上了、到了的照常回");
}
