//! 分位数的取法、表里一格的写法。

use super::*;

#[test]
fn nearest_rank() {
    let values: Vec<f64> = (1..=100).rev().map(f64::from).collect();
    assert_eq!(percentile(&values, 50.0), Some(50.0));
    assert_eq!(percentile(&values, 95.0), Some(95.0));
    assert_eq!(percentile(&values, 99.0), Some(99.0));
    assert_eq!(percentile(&values, 100.0), Some(100.0));
    assert_eq!(percentile(&[3.0, 1.0, 2.0], 50.0), Some(2.0));
    assert_eq!(percentile(&[7.0], 99.0), Some(7.0));
    assert_eq!(percentile(&[7.0, 9.0], 0.0), Some(7.0));
    assert_eq!(percentile(&[], 50.0), None);
}

#[test]
fn a_cell() {
    assert_eq!(spread(&[1.0, 2.0, 3.0, 4.0]), "2.0 / 4.0");
    assert_eq!(spread(&[]), "—");
    assert_eq!(ms(Duration::from_micros(1500)), 1.5);
}
