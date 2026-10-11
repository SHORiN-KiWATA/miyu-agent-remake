//! 人看得懂的大小（施工 O-33）：几个单位的边界、四舍五入、进位到下一个单位、最大的数。

use super::readable;

#[test]
fn under_a_thousand_bytes_is_written_in_bytes() {
    assert_eq!(readable(0), "0 B");
    assert_eq!(readable(1), "1 B");
    assert_eq!(readable(999), "999 B");
}

#[test]
fn kilobytes_are_whole_and_rounded_half_up() {
    assert_eq!(readable(1000), "1 KB");
    assert_eq!(readable(1499), "1 KB");
    assert_eq!(readable(1500), "2 KB");
    assert_eq!(readable(834_000), "834 KB");
    assert_eq!(readable(834_499), "834 KB");
    assert_eq!(readable(999_499), "999 KB");
}

#[test]
fn a_kilobyte_count_that_rounds_to_a_thousand_is_a_megabyte() {
    // 999.5 KB、999.95 KB、1000 KB 都进位成 1.0 MB，不写「1000 KB」。
    assert_eq!(readable(999_500), "1.0 MB");
    assert_eq!(readable(999_950), "1.0 MB");
    assert_eq!(readable(1_000_000), "1.0 MB");
}

#[test]
fn megabytes_and_gigabytes_have_one_decimal_rounded_half_up() {
    assert_eq!(readable(1_234_567), "1.2 MB");
    assert_eq!(readable(1_249_999), "1.2 MB");
    assert_eq!(readable(1_250_000), "1.3 MB");
    assert_eq!(readable(12_345_678), "12.3 MB");
    assert_eq!(readable(999_949_999), "999.9 MB");
    // 999.95 MB 进位成 1.0 GB。
    assert_eq!(readable(999_950_000), "1.0 GB");
    assert_eq!(readable(1_000_000_000), "1.0 GB");
    assert_eq!(readable(12_345_678_901), "12.3 GB");
    assert_eq!(readable(1_000_000_000_000), "1000.0 GB");
}

#[test]
fn the_largest_count_does_not_overflow() {
    assert_eq!(readable(u64::MAX), "18446744073.7 GB");
}
