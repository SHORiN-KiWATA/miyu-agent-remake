//! 超了多少的测试（施工 6-6 中）：几家报错的原话照小写以后找。

use super::{excess, limit};

#[test]
fn each_known_wording_gives_how_many_tokens_over() {
    for (said, over) in [
        // OpenAI 的写法。
        (
            "This model's maximum context length is 128000 tokens. However, your messages resulted in 130000 tokens. Please reduce the length of the messages.",
            Some(2_000),
        ),
        // DeepSeek 的写法：requested 里算上了要写的输出。
        (
            "This model's maximum context length is 131072 tokens. However, you requested 140000 tokens (135904 in the messages, 4096 in the completion). Please reduce the length of the messages or completion.",
            Some(8_928),
        ),
        // Anthropic 的写法，带千分位。
        (
            "prompt is too long: 213,462 tokens > 200,000 maximum",
            Some(13_462),
        ),
        // 没超、说不出数、别的写法。
        (
            "This model's maximum context length is 65536 tokens. However, you requested 60000 tokens.",
            None,
        ),
        ("maximum context length is exceeded", None),
        ("Request too large for model", None),
        ("prompt is too long", None),
    ] {
        assert_eq!(excess(&said.to_lowercase()), over, "{said}");
    }
}

/// 报了的上限（施工 8-7）：只要说得出 N 就交，后半段说不出、没超的也交；说不出 N 的、0 不交。
#[test]
fn the_stated_limit_is_read_on_its_own() {
    for (said, stated) in [
        (
            "This model's maximum context length is 128000 tokens. However, your messages resulted in 130000 tokens.",
            Some(128_000),
        ),
        (
            "This model's maximum context length is 65536 tokens. However, you requested 60000 tokens.",
            Some(65_536),
        ),
        ("maximum context length is 8,192 tokens", Some(8_192)),
        (
            "prompt is too long: 213,462 tokens > 200,000 maximum",
            Some(200_000),
        ),
        ("prompt is too long: 213462 tokens", None),
        ("maximum context length is exceeded", None),
        ("maximum context length is 0 tokens", None),
        ("Request too large for model", None),
    ] {
        assert_eq!(limit(&said.to_lowercase()), stated, "{said}");
    }
}
