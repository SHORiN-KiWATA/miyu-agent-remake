//! 超长的超了多少 token（`docs/blueprint/drivers/openai-chat.md`「出错怎么分」第 7 条，施工 6-6 中）：从找说法的字里
//! 解析，交给内核截短摘要请求（`compaction.md` 第三条第 10 条）。报了的上限另交出来（施工 8-7）：执行器照它记下用出来的
//! 窗口（`models.md`「怎么走」第二条第 9 条）。纯逻辑层没有正则，照几家常见的写法一段一段找。

/// 超了多少 token：`text` 是小写以后找说法的字。先对上的算：
///
/// - `maximum context length is <N>` 后面跟着 `resulted in <M>` 或者 `requested <M>`（OpenAI、DeepSeek）；
/// - `prompt is too long: <M> tokens > <N>`（Anthropic）。
///
/// M 比 N 大才算，交回 M 减 N；别的都没有。
pub(super) fn excess(text: &str) -> Option<u64> {
    context_length(text).or_else(|| prompt_too_long(text))
}

/// 报了的上限 N（施工 8-7）：`text` 是小写以后找说法的字，写法同 [`excess`]，只要说得出 N，后半段说不出也算。0 不算。
pub(super) fn limit(text: &str) -> Option<u64> {
    let after = |phrase: &str| text.split_once(phrase).map(|(_, rest)| rest);
    let stated = after("maximum context length is")
        .and_then(number)
        .or_else(|| {
            let (_, rest) = number(after("prompt is too long:")?)?;
            number(rest.split_once('>')?.1)
        });
    stated.map(|(limit, _)| limit).filter(|limit| *limit > 0)
}

/// `maximum context length is <N> … resulted in <M>` / `… requested <M>`。
fn context_length(text: &str) -> Option<u64> {
    let (_, rest) = text.split_once("maximum context length is")?;
    let (limit, rest) = number(rest)?;
    let wanted = ["resulted in", "requested"]
        .iter()
        .filter_map(|phrase| rest.find(phrase).map(|at| (at, phrase.len())))
        .min()
        .and_then(|(at, len)| number(&rest[at + len..]))
        .map(|(wanted, _)| wanted)?;
    wanted.checked_sub(limit).filter(|over| *over > 0)
}

/// `prompt is too long: <M> tokens > <N>`。
fn prompt_too_long(text: &str) -> Option<u64> {
    let (_, rest) = text.split_once("prompt is too long:")?;
    let (wanted, rest) = number(rest)?;
    let (_, rest) = rest.split_once('>')?;
    let (limit, _) = number(rest)?;
    wanted.checked_sub(limit).filter(|over| *over > 0)
}

/// 跳过前面的空白，读一个整数，数字中间可以有千分位的逗号；交回数和后面剩下的。前面不是数字的没有。
fn number(text: &str) -> Option<(u64, &str)> {
    let text = text.trim_start();
    let mut value: u64 = 0;
    let mut end = 0;
    let bytes = text.as_bytes();
    while end < bytes.len() {
        match bytes[end] {
            digit @ b'0'..=b'9' => {
                value = value
                    .checked_mul(10)?
                    .checked_add(u64::from(digit - b'0'))?;
                end += 1;
            }
            b',' if end > 0 && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) => end += 1,
            _ => break,
        }
    }
    (end > 0).then(|| (value, &text[end..]))
}

#[cfg(test)]
mod tests;
