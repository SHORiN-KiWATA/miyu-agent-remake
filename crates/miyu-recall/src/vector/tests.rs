//! 向量的写法（施工 R-5 下）：写成字节读回一位不差、长度是维数的 4 倍；长度不是 4 的倍数的读不出；点积。

use super::*;

#[test]
fn bytes_round_trip_exactly() {
    let vector = vec![0.1_f32, -0.25, 1.0e-7, 0.0, f32::MIN_POSITIVE];
    let bytes = to_bytes(&vector);
    assert_eq!(bytes.len(), 4 * vector.len());
    assert_eq!(&bytes[..4], &0.1_f32.to_le_bytes(), "小端");
    assert_eq!(from_bytes(&bytes), Some(vector));
    assert_eq!(from_bytes(&[]), Some(Vec::new()));
    assert_eq!(from_bytes(&[1, 2, 3]), None, "不是 4 的倍数");
}

#[test]
fn similarity_is_the_dot_product() {
    assert_eq!(dot(&[0.6, 0.8], &[0.6, 0.8]), 1.0);
    assert_eq!(dot(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
    assert!((dot(&[0.6, 0.8], &[0.8, 0.6]) - 0.96).abs() < 1e-6);
    assert_eq!(dot(&[1.0, 2.0], &[1.0]), 0.0, "维数对不上的不算相似");
}
