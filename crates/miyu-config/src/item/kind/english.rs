//! 给模型看的字（`docs/blueprint/config.md`「类型」，施工 8-8 补）：一行英文。给模型看的字一律英文（`26-提示词.md` J3），
//! 写进配置的也一样：池的说明会拼进 `subagent` 的参数里。
//!
//! CJK 的字照 Unicode 的几段认（汉字连同扩展区和兼容汉字、假名、谚文、CJK 的标点、全角的字），字数乘二不小于总字数就不收：
//! 「占一半以上」含一半，宁严（`config.md`「施工时定的」8-8 补）。几段是写死的范围，规则本身简单，不另引依赖。

/// `text` 合不合给模型看的字：不是空的，最多 `max` 个字符，没有控制字符（连同换行：只能一行），CJK 的字不到一半。
pub(super) fn english(text: &str, max: usize) -> bool {
    let count = text.chars().count();
    let cjk = text.chars().filter(|c| is_cjk(*c)).count();
    count > 0 && count <= max && !text.chars().any(char::is_control) && cjk * 2 < count
}

/// 是不是 CJK 的字。
fn is_cjk(c: char) -> bool {
    matches!(
        u32::from(c),
        0x1100..=0x11FF // 谚文字母
            | 0x2E80..=0x2FDF // 部首
            | 0x3000..=0x303F // CJK 的标点
            | 0x3040..=0x30FF // 平假名、片假名
            | 0x3130..=0x318F // 谚文兼容字母
            | 0x31F0..=0x31FF // 片假名扩展
            | 0x3400..=0x4DBF // 汉字扩展 A
            | 0x4E00..=0x9FFF // 汉字
            | 0xAC00..=0xD7AF // 谚文音节
            | 0xF900..=0xFAFF // 兼容汉字
            | 0xFF00..=0xFFEF // 全角、半角的字
            | 0x20000..=0x3134F // 汉字扩展 B 以后
    )
}

#[cfg(test)]
mod tests;
