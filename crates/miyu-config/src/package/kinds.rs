//! 包带了什么照表认（施工 F-8 上补，设计 `31-软件包.md` 第二节第 1、2 条）：清单不写种类。程序表 `[ui]`、`[process]`、
//! `[builtin]`、`[worker]` 至多一张，有哪一张就是哪一种；没有程序表、有 `[mascot]` 的是吉祥物包；都没有的少了。哪种包能写
//! 哪几张表照 [`tables`]，吉祥物哪一种都能带。

use toml_edit::Table;

use super::{Code, PackageKind, Problem, Reader};

/// 程序表和它对着的种类，照这个先后报。
const PROGRAMS: [(&str, PackageKind); 4] = [
    ("ui", PackageKind::Ui),
    ("process", PackageKind::Process),
    ("builtin", PackageKind::Builtin),
    ("worker", PackageKind::Worker),
];

/// 照清单 `root` 里有哪几张表认出种类。
pub(super) fn of(reader: &Reader<'_>, root: &Table) -> Result<PackageKind, Problem> {
    let found: Vec<(&str, PackageKind)> = PROGRAMS
        .into_iter()
        .filter(|(name, _)| root.contains_key(name))
        .collect();
    match found.as_slice() {
        [(_, kind)] => Ok(*kind),
        [] if root.contains_key("mascot") => Ok(PackageKind::Mascot),
        [] => Err(reader.problem(
            None,
            Code::MissingKey,
            "[ui] / [process] / [builtin] / [worker] / [mascot]",
            "a package carries a program ([ui], [process], [builtin] or [worker]) or a [mascot]"
                .to_string(),
        )),
        [(first, _), (second, _), ..] => Err(reader.problem(
            Some(&root[second]),
            Code::TwoPrograms,
            &format!("[{first}], [{second}]"),
            format!("a package carries one program: [{first}] and [{second}] can't both be here"),
        )),
    }
}

/// 这种包能写哪几张表（施工 F-1；F-8 上补起吉祥物哪一种都能带）：别的写了报 `wrong_kind`。
pub(super) fn tables(kind: PackageKind) -> &'static [&'static str] {
    match kind {
        PackageKind::Ui => &[
            "package",
            "command",
            "ui",
            "check",
            "settings",
            "depends",
            "recommends",
            "mascot",
        ],
        PackageKind::Process => &[
            "package",
            "command",
            "process",
            "check",
            "settings",
            "features",
            "connection",
            "depends",
            "recommends",
            "page",
            "mascot",
        ],
        PackageKind::Builtin => &[
            "package",
            "builtin",
            "features",
            "depends",
            "recommends",
            "page",
            "mascot",
        ],
        PackageKind::Worker => &["package", "worker", "mascot"],
        PackageKind::Mascot => &["package", "mascot"],
    }
}
