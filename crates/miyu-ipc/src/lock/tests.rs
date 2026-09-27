use super::*;
use crate::test_support::Scratch;

#[test]
fn a_second_lock_waits_for_the_first_to_go() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let first = Lock::acquire(&root).expect("第一把拿得到");
    assert!(root.run().join(FILE).is_file());
    let error = Lock::acquire(&root).expect_err("第二把拿不到");
    assert!(matches!(error, OpenError::Running), "{error:?}");
    drop(first);
    Lock::acquire(&root).expect("第一把放开以后拿得到");
}

#[test]
fn two_data_roots_do_not_share_a_lock() {
    let (one, two) = (Scratch::new(), Scratch::new());
    let _first = Lock::acquire(&one.root()).expect("拿得到");
    Lock::acquire(&two.root()).expect("另一个数据根照样拿得到");
}
