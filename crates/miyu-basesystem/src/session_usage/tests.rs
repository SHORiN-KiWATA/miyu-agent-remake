//! 金额给她看的写法（施工 8-15）：三位有效数字、至少两位小数，多出来的 0 去掉。

use super::money;

#[test]
fn money_keeps_three_significant_digits_and_at_least_two_decimals() {
    for (amount, written) in [
        (0.42, "0.42"),
        (1.3, "1.30"),
        (0.000_292_05, "0.000292"),
        (0.0123, "0.0123"),
        (0.5, "0.50"),
        (12.0, "12.00"),
        (1234.5678, "1234.57"),
        (0.0, "0.00"),
        (0.000_001_234, "0.00000123"),
    ] {
        assert_eq!(money(amount), written, "{amount}");
    }
}
