use super::*;

/// 真机上三个平台都装着字体（CI 的三平台机器也是）：读得到至少一种。
#[test]
fn this_machine_has_at_least_one_font() {
    assert!(available(), "这台机器上一种字体都读不到，装一个再跑测试");
}
