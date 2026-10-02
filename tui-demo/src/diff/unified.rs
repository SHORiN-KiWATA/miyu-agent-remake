//! 核心给的统一格式差异（撤销的回应里 `files` 的 `diff`，`protocol/undo.md`）读成 [`Diff`]：`@@ -a,b +c,d @@` 定起始
//! 行号，` ` 不变、`-` 删掉、`+` 加上；两段之间是一行略过的。读不懂的行不要。

use super::{Diff, Line, Mark};

/// 读几行统一格式的差异。
pub fn read(lines: &[String]) -> Diff {
    let mut out = Diff::default();
    let (mut old, mut new) = (0usize, 0usize);
    for line in lines {
        if let Some(header) = line.strip_prefix("@@") {
            if !out.lines.is_empty() {
                out.lines.push(Line {
                    mark: Mark::Gap,
                    number: None,
                    text: String::new(),
                });
            }
            (old, new) = starts(header);
            continue;
        }
        let mut chars = line.chars();
        let (mark, number) = match chars.next() {
            Some('-') => {
                out.removed += 1;
                old += 1;
                (Mark::Removed, old - 1)
            }
            Some('+') => {
                out.added += 1;
                new += 1;
                (Mark::Added, new - 1)
            }
            Some(' ') | None => {
                old += 1;
                new += 1;
                (Mark::Keep, new - 1)
            }
            _ => continue,
        };
        out.lines.push(Line {
            mark,
            number: Some(number),
            text: chars.as_str().to_string(),
        });
    }
    out
}

/// `-3,2 +3,4 @@` 里两边起始的行号。
fn starts(header: &str) -> (usize, usize) {
    let mut old = 1;
    let mut new = 1;
    for part in header.split_whitespace() {
        let number = |p: &str| {
            p.split(',')
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(1)
        };
        if let Some(p) = part.strip_prefix('-') {
            old = number(p);
        } else if let Some(p) = part.strip_prefix('+') {
            new = number(p);
        }
    }
    (old, new)
}

#[cfg(test)]
mod tests {
    use super::read;
    use crate::diff::Mark;

    #[test]
    fn hunks_number_their_lines_and_a_gap_sits_between_them() {
        let lines: Vec<String> = [
            "@@ -3,2 +3,2 @@",
            " keep",
            "-fn main() {}",
            "+fn main() { println!(\"hi\"); }",
            "@@ -20 +20,2 @@",
            "+added",
        ]
        .map(str::to_string)
        .to_vec();
        let diff = read(&lines);
        let got: Vec<(Mark, Option<usize>)> =
            diff.lines.iter().map(|l| (l.mark, l.number)).collect();
        assert_eq!(
            got,
            [
                (Mark::Keep, Some(3)),
                (Mark::Removed, Some(4)),
                (Mark::Added, Some(4)),
                (Mark::Gap, None),
                (Mark::Added, Some(20)),
            ]
        );
        assert_eq!((diff.added, diff.removed), (2, 1));
        assert_eq!(diff.lines[1].text, "fn main() {}");
    }
}
