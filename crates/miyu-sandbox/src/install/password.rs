//! 沙盒用户的随机密码（`docs/blueprint/sandbox/windows.md`「沙盒用户」）：32 个字符，大写、小写、数字、符号各至少一个，
//! 其余从这四类里随机取。四类都有，开着密码复杂度策略的机器上也建得成。
//!
//! 字节到字符用取余：会偏向表里前面的几个字，每个字符少不到一比特，32 个字符加起来还有一百多比特，不值得为它写拒绝
//! 采样。

/// 密码有几个字符。
pub(crate) const LENGTH: usize = 32;

/// 大写字母。
pub(crate) const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// 小写字母。
pub(crate) const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
/// 数字。
pub(crate) const DIGITS: &[u8] = b"0123456789";
/// 符号：不含引号、反斜杠、空格，哪里转手都不用转义。
pub(crate) const SYMBOLS: &[u8] = b"!#%+-.:=?@^_~";

/// 照 32 个随机字节拼一个密码：前四个字符依次从大写、小写、数字、符号里取，其余从四类合起来的表里取。每个字节都
/// 用上，字节不同，密码就不同。
pub(crate) fn generate(random: &[u8; LENGTH]) -> String {
    let all: Vec<u8> = [UPPER, LOWER, DIGITS, SYMBOLS].concat();
    let pick = |set: &[u8], byte: u8| char::from(set[usize::from(byte) % set.len()]);
    let (first, rest) = random.split_at(4);
    let each_kind = [UPPER, LOWER, DIGITS, SYMBOLS]
        .iter()
        .zip(first)
        .map(|(set, byte)| pick(set, *byte));
    each_kind
        .chain(rest.iter().map(|byte| pick(&all, *byte)))
        .collect()
}

#[cfg(test)]
mod tests;
