//! 超了多少的测试（施工 6-6 中）：几家报错的原话照小写以后找。

use super::excess;

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
