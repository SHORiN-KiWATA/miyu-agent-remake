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
