//! 测试共用的几样：仓库里的出厂文件（`include_str!` 读进来：数据改了，测试跟着变），包成一份 [`File`]、读成 [`Params`]；
//! 改一处再读、读出来的问题怎么比（`tests.rs`、`lists_tests.rs` 共用）。

use miyu_config::problem::{At, Code};

use crate::Params;
use crate::rules::{File, Problem, Source};

/// 仓库里的出厂文件。
pub(crate) const DEFAULTS: &str =
    include_str!("../../../../resources/software/onebot/defaults.toml");

/// 出厂文件的名字：问题里的 `file` 照交进来的。
pub(crate) const NAME: &str = "defaults.toml";

/// 一份出厂文件，字是 `text`。
pub(crate) fn file(text: &str) -> File {
    File {
        source: Source::Factory,
        name: NAME.to_string(),
        text: text.to_string(),
    }
}

/// 仓库里的出厂文件读出来的参数，断定读得出。
pub(crate) fn defaults() -> Params {
    Params::read(&file(DEFAULTS)).expect("仓库里的出厂文件读得出")
}

/// 仓库里的出厂文件改一处：`from` 正好出现一次，换成 `to`。
pub(crate) fn edited(from: &str, to: &str) -> String {
    assert_eq!(DEFAULTS.matches(from).count(), 1, "{from} 要正好出现一次");
    DEFAULTS.replace(from, to)
}

/// 一条问题看的几样：原因码、键、位置、收到的、最近的名字。
pub(crate) type Seen = (
    Code,
    Option<String>,
    Option<At>,
    Option<String>,
    Option<String>,
);

/// 读 `text`，断定有问题，交回每一条看的几样；顺带断定每一条都照交进来的文件、没有第几条规则。
pub(crate) fn problems(text: &str) -> Vec<Seen> {
    let problems: Vec<Problem> = Params::read(&file(text)).expect_err("应当有问题");
    for problem in &problems {
        assert_eq!(problem.source, Source::Factory);
        assert_eq!(problem.file, NAME);
        assert_eq!(problem.rule, None);
    }
    problems
        .into_iter()
        .map(|p| (p.code, p.key, p.at, p.got, p.suggest))
        .collect()
}

/// 一项的问题：值写错了，指到值。
pub(crate) fn bad(code: Code, key: &str, line: usize, column: usize, got: &str) -> Seen {
    (
        code,
        Some(key.into()),
        Some(At { line, column }),
        Some(got.into()),
        None,
    )
}

/// 缺了的一项：没有位置、没有收到的。
pub(crate) fn missing(key: &str) -> Seen {
    (Code::WrongType, Some(key.into()), None, None, None)
}

/// 出厂文件里 `needle` 在第几行，从 1 数。
pub(crate) fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.starts_with(needle))
        .map(|index| index + 1)
        .expect(needle)
}

/// 读出厂文件改过的一份里某一项的原因码：断定只有这一条问题，键是 `key`。
pub(crate) fn code_of(text: &str, key: &str) -> Code {
    let seen = problems(text);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].1.as_deref(), Some(key));
    seen[0].0
}
