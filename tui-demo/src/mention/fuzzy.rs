//! 模糊找怎么排（蓝图 `tui.md`「`@` 文件列表」第 3 条）：打的字照先后都在路径里（不论大小写）的才算；落在文件名里的、
//! 在一段开头的、连着的、文件名（去掉扩展名）正好是打的字的分高，路径短的在前。

/// 路径里分段的字：它后面的字算一段的开头。
const SEPARATORS: [char; 5] = ['/', '-', '_', '.', ' '];

/// `query` 照先后都在 `path` 里：交回分数和对上的是第几个字（按字符数）；对不上的是 `None`。先在文件名里找，找不全再
/// 从头找。
pub fn score(query: &str, path: &str) -> Option<(i64, Vec<usize>)> {
    let query: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let chars: Vec<char> = path.chars().collect();
    let lower: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let trimmed = path.trim_end_matches('/').chars().count();
    let name_start = chars[..trimmed]
        .iter()
        .rposition(|c| *c == '/')
        .map_or(0, |at| at + 1);
    let found = find(&query, &lower, name_start).or_else(|| find(&query, &lower, 0))?;
    let mut total = 0i64;
    for (k, &at) in found.iter().enumerate() {
        total += 10;
        if at == 0 || SEPARATORS.contains(&chars[at - 1]) {
            total += 8;
        }
        if k > 0 && found[k - 1] + 1 == at {
            total += 6;
        }
        if at >= name_start {
            total += 4;
        }
    }
    // 文件名去掉扩展名正好是打的字（打 `main` 找 `main.rs`）。
    let name: String = lower[name_start..trimmed].iter().collect();
    let stem = name
        .rsplit_once('.')
        .map_or(name.as_str(), |(stem, _)| stem);
    if !stem.is_empty() && stem.chars().eq(query.iter().copied()) {
        total += 30;
    }
    let length = i64::try_from(chars.len()).unwrap_or(i64::MAX);
    Some((total * 100 - length, found))
}

/// 从 `from` 起照先后找每一个字，交回位置。
fn find(query: &[char], lower: &[char], from: usize) -> Option<Vec<usize>> {
    let mut at = from;
    let mut out = Vec::with_capacity(query.len());
    for q in query {
        let offset = lower.get(at..)?.iter().position(|c| c == q)?;
        out.push(at + offset);
        at += offset + 1;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::score;

    fn rank<'a>(query: &str, paths: &[&'a str]) -> Vec<&'a str> {
        let mut found: Vec<(i64, &str)> = paths
            .iter()
            .filter_map(|p| score(query, p).map(|(s, _)| (s, *p)))
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        found.into_iter().map(|(_, p)| p).collect()
    }

    #[test]
    fn names_segment_starts_and_short_paths_come_first() {
        let paths = [
            "docs/main-notes.md",
            "src/main.rs",
            "tui-demo/src/main.rs",
            "src/domain.rs",
            "README.md",
        ];
        assert_eq!(
            rank("main", &paths),
            [
                "src/main.rs",
                "tui-demo/src/main.rs",
                "docs/main-notes.md",
                "src/domain.rs"
            ]
        );
        assert_eq!(rank("Read", &paths), ["README.md"], "不论大小写");
        assert!(rank("xyz", &paths).is_empty());
        let (_, at) = score("sm", "src/main.rs").unwrap();
        assert_eq!(at, [0, 4], "文件名里只有 m，找不全就从头找");
    }
}
