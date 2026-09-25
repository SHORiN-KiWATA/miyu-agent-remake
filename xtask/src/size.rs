//! 行数门禁：每个 `.rs` 文件不超过图纸上定的行数（01-架构 第九节，规则 4）。

/// 超了就说一句；没超返回 `None`。
pub fn check(label: &str, text: &str, max: usize) -> Option<String> {
    let lines = text.lines().count();
    (lines > max)
        .then(|| format!("{label} 有 {lines} 行，图纸定的上限是 {max} 行：把它拆开（规则 4）"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_over_limit_is_rejected() {
        let text = "x\n".repeat(501);
        let problem = check("big.rs", &text, 500).unwrap();
        assert!(problem.contains("501 行"), "{problem}");
    }

    #[test]
    fn file_at_limit_passes() {
        assert_eq!(check("ok.rs", &"x\n".repeat(500), 500), None);
    }
}
