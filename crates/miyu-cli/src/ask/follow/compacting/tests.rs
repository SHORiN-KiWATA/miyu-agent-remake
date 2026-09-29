//! token 数的写法（施工 6-3 下）。

use super::*;

#[test]
fn tokens_are_written_in_k_and_m_with_one_decimal() {
    for (n, shown) in [
        (0, "0"),
        (999, "999"),
        (1_000, "1k"),
        (31_020, "31k"),
        (812_345, "812.3k"),
        (1_000_000, "1M"),
        (1_234_567, "1.2M"),
    ] {
        assert_eq!(tokens(n), shown, "{n}");
    }
}

#[test]
fn nothing_written_yet_has_no_count() {
    // 摘要请求刚发出去、还没收到字的不写字数（施工 6-3 三补，和终端界面一样）。
    assert_eq!(progress(&Language::Chinese, 0), "· 正在压缩上下文…");
    assert_eq!(progress(&Language::English, 0), "· Compacting the context…");
    assert_eq!(
        progress(&Language::Chinese, 1),
        "· 正在压缩上下文… 已写 1 字"
    );
    assert_eq!(
        progress(&Language::English, 3_120),
        "· Compacting the context… 3,120 characters written"
    );
}
