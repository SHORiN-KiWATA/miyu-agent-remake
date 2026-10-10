//! 人看得懂的大小（施工 O-33，`docs/blueprint/kernel/request.md`「群里的一行」）：群里的一行写带的东西多大。十进制单位：
//! 不到 1000 字节写 `512 B`，KB 写整数，MB、GB 一位小数，都四舍五入；四舍五入到下一个单位的那一档（999.5 KB、
//! 999.95 MB）写成下一个单位（`1.0 MB`、`1.0 GB`）。只用整数算：三个平台写出一样的字节，不靠浮点格式化的默认值。

/// 一千：十进制单位之间差的倍数。
const THOUSAND: u128 = 1000;

/// `bytes` 个字节写成人看得懂的大小，例如 `834 KB`、`1.2 MB`。
pub(crate) fn readable(bytes: u64) -> String {
    let bytes = u128::from(bytes);
    if bytes < THOUSAND {
        return format!("{bytes} B");
    }
    let kilobytes = rounded(bytes, THOUSAND);
    if kilobytes < THOUSAND {
        return format!("{kilobytes} KB");
    }
    // 一位小数：先算成十分之一个单位，四舍五入。
    let megabytes = rounded(bytes, THOUSAND * THOUSAND / 10);
    if megabytes < THOUSAND * 10 {
        return tenths(megabytes, "MB");
    }
    tenths(rounded(bytes, THOUSAND * THOUSAND * THOUSAND / 10), "GB")
}

/// `bytes` 除以 `unit`，四舍五入到整数。用 `u128` 算，加半个单位不会溢出。
fn rounded(bytes: u128, unit: u128) -> u128 {
    (bytes + unit / 2) / unit
}

/// 十分之一个单位的个数写成一位小数。
fn tenths(count: u128, unit: &str) -> String {
    format!("{}.{} {unit}", count / 10, count % 10)
}

#[cfg(test)]
mod tests;
