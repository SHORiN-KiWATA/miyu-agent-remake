//! 能不能用：协议上的写法、读回来、能用的才有助手（施工 5-4 下）。

use super::*;

#[test]
fn each_reason_has_its_code_and_reads_back() {
    let codes: Vec<&str> = Unusable::ALL.iter().map(|reason| reason.code()).collect();
    assert_eq!(codes, ["helper_missing", "helper_failed", "no_mechanism"]);
    for reason in Unusable::ALL {
        assert_eq!(Unusable::from_code(reason.code()), Some(reason));
    }
    assert_eq!(Unusable::from_code("no_landlock"), None, "不认得的是空的");
}

#[test]
fn only_a_usable_sandbox_has_a_helper() {
    let helper = PathBuf::from("/opt/miyu/miyu-sandbox");
    assert_eq!(
        Availability::Usable(helper.clone()).helper(),
        Some(helper.as_path())
    );
    for reason in Unusable::ALL {
        assert_eq!(Availability::Unusable(reason).helper(), None);
    }
}
