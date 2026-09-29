//! 官方计算器 v41 配置的对照数据（照 dsh 测试里的，2026-09-28 的版本）。

use super::*;

#[test]
fn the_calculator_reference_values() {
    for (width, height, tokens) in [
        (100, 100, 184),
        (544, 544, 184),
        (640, 480, 206),
        (800, 800, 422),
        (1024, 768, 496),
        (1066, 600, 407),
        (1300, 1300, 994),
        (1920, 1080, 968),
        (2000, 2000, 994),
        (5000, 5000, 994),
        (300, 50, 200),
        (8192, 100, 593),
        (16, 8192, 590),
    ] {
        assert_eq!(
            deepseek_image_tokens(width, height),
            tokens,
            "{width}×{height}"
        );
    }
}

#[test]
fn every_image_is_capped_at_1024() {
    for (width, height) in [
        (2000, 2000),
        (5000, 5000),
        (8192, 8192),
        (16, 8192),
        (9000, 1),
        (1, 9000),
    ] {
        assert!(
            deepseek_image_tokens(width, height) <= 1024,
            "{width}×{height}"
        );
    }
    // 一行、一列的极端形状，解出来正好是上限。
    assert_eq!(deepseek_image_tokens(9000, 1), 1024);
    assert_eq!(deepseek_image_tokens(1, 9000), 1024);
}

#[test]
fn small_images_cost_the_same_as_the_scale_up_floor() {
    assert_eq!(
        deepseek_image_tokens(100, 100),
        deepseek_image_tokens(544, 544)
    );
}

#[test]
fn it_takes_several_passes_to_settle_on_some_shapes() {
    assert_eq!(deepseek_image_tokens(12, 1123), 380);
    assert_eq!(deepseek_image_tokens(89, 2076), 254);
    // 这几组第一遍投影出来的 token 数和收敛以后的不一样（照同一个算法算的，不是官方计算器给的数）：只投影一遍就错。
    assert_eq!(deepseek_image_tokens(7, 1123), 338);
    assert_eq!(deepseek_image_tokens(41, 1), 172);
    assert_eq!(deepseek_image_tokens(14, 5000), 506);
}

#[test]
fn it_is_the_image_price_of_the_kernel() {
    assert_eq!(DeepSeekImages.tokens(1920, 1080), 968);
}
