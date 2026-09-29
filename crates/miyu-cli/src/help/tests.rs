//! 帮助页是手写的，得和程序真有的选项对得上（施工 4-11）：每一页列的选项、要不要写值、能写哪些值，和 clap 照参数
//! 定义生成的一一对上；最宽 80 列，以一个换行结尾。

use std::collections::BTreeSet;

use clap::{Args, Command};

use super::{Page, page};
use crate::language::Language;
use crate::{Ask, Compact, Redo, Sandbox, Undo};

/// 一个选项：几种写法（`-c`、`--continue`），和后面写的值（没有的是空的）。
type Listed = BTreeSet<(Vec<String>, String)>;

/// 一页里列的选项：左边一栏以 `-` 开头的那些行，左边一栏到连着两个空格为止。
fn listed(page: &str) -> Listed {
    let mut options = Listed::new();
    for row in page.lines() {
        let row = row.trim_start();
        if !row.starts_with('-') {
            continue;
        }
        let left = row.split("  ").next().unwrap_or_default();
        let mut names = Vec::new();
        let mut value = String::new();
        for word in left.split_whitespace() {
            match word.trim_end_matches(',') {
                name if name.starts_with('-') => names.push(name.to_string()),
                shown => value = shown.to_string(),
            }
        }
        names.sort();
        options.insert((names, value));
    }
    options
}

/// 程序真有的选项：`command` 里有名字的参数，加上 clap 给每条命令都加的 `-h`、`--help`。要写值的，值写成能写的几样
/// 用 `|` 连起来；不限的照 clap 的值名在 `values` 里查页里写成什么（`SESSION` 是 `<编号>`，`DIR` 是 `<目录>`，施工 5-10
/// 上；`FILE` 是 `<文件>`，施工 3-9 三补），查不到的写成 `<值名>`。藏起来的不算：它们不给人用（`sandbox` 那两个，施工 5-8）。
fn real(command: &Command, values: &[(&str, &str)]) -> Listed {
    let mut options = Listed::new();
    for arg in command.get_arguments().filter(|arg| !arg.is_hide_set()) {
        let mut names: Vec<String> = arg
            .get_short()
            .map(|c| format!("-{c}"))
            .into_iter()
            .collect();
        names.extend(arg.get_long().map(|long| format!("--{long}")));
        if names.is_empty() {
            continue;
        }
        names.sort();
        let shown = match arg.get_action().takes_values() {
            false => String::new(),
            true => {
                let possible: Vec<String> = arg
                    .get_possible_values()
                    .iter()
                    .map(|possible| possible.get_name().to_string())
                    .collect();
                match possible.is_empty() {
                    true => {
                        let name = arg
                            .get_value_names()
                            .and_then(|names| names.first())
                            .map(ToString::to_string)
                            .unwrap_or_default();
                        values
                            .iter()
                            .find(|(from, _)| *from == name)
                            .map_or_else(|| format!("<{name}>"), |(_, to)| (*to).to_string())
                    }
                    false => possible.join("|"),
                }
            }
        };
        options.insert((names, shown));
    }
    options.insert((vec!["--help".to_string(), "-h".to_string()], String::new()));
    options
}

/// `miyu ask`、`miyu undo`（`restore` 一样）的参数定义。
fn commands() -> (Command, Command) {
    (
        Ask::augment_args(Command::new("ask")),
        Undo::augment_args(Command::new("undo")),
    )
}

#[test]
fn each_page_lists_exactly_the_options_there_are() {
    let (ask, undo) = commands();
    let chinese = [("SESSION", "<编号>"), ("DIR", "<目录>"), ("FILE", "<文件>")];
    let english = [("SESSION", "<id>"), ("DIR", "<dir>"), ("FILE", "<file>")];
    for (language, id) in [(Language::Chinese, &chinese), (Language::English, &english)] {
        assert_eq!(
            listed(page(language, Page::Ask)),
            real(&ask, id),
            "{language:?} ask"
        );
        for which in [Page::Undo, Page::Restore] {
            assert_eq!(
                listed(page(language, which)),
                real(&undo, id),
                "{language:?} {which:?}"
            );
        }
        // `compact` 那一页（施工 6-8）：和 `undo` 一样只有 `-s`，要求是位置参数，不算选项。
        let compact = Compact::augment_args(Command::new("compact"));
        assert_eq!(
            listed(page(language, Page::Compact)),
            real(&compact, id),
            "{language:?} compact"
        );
        // `redo` 那一页（施工 4-7 再补）：和 `compact` 一样只有 `-s`，换成的话是位置参数。
        let redo = Redo::augment_args(Command::new("redo"));
        assert_eq!(
            listed(page(language, Page::Redo)),
            real(&redo, id),
            "{language:?} redo"
        );
        // 主程序那一页：`ask` 的、`undo`、`restore`、`redo`、`compact` 的都列，再加 `-V`、`--version`。
        let mut all = real(&ask, id);
        all.extend(real(&undo, id));
        all.extend(real(&redo, id));
        all.extend(real(&compact, id));
        all.insert((
            vec!["--version".to_string(), "-V".to_string()],
            String::new(),
        ));
        assert_eq!(listed(page(language, Page::Miyu)), all, "{language:?} miyu");
        // `sandbox` 那一页：`sandbox`、`sandbox setup`、`sandbox remove` 印的都是它。
        let sandbox = Sandbox::augment_args(Command::new("sandbox"));
        let mut commands = vec![sandbox.clone()];
        commands.extend(sandbox.get_subcommands().cloned());
        assert_eq!(commands.len(), 3, "setup、remove 两个子命令");
        for command in commands {
            assert_eq!(
                listed(page(language, Page::Sandbox)),
                real(&command, id),
                "{language:?} {}",
                command.get_name()
            );
        }
    }
}

#[test]
fn each_page_is_its_own_file() {
    // 照文件名去读盘上的那一份比，不拿 `page` 自己当答案：哪两页接错了，这里红（变异测试逮到过 undo 印成恢复那一页）。
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/help");
    for (language, code) in [(Language::Chinese, "zh"), (Language::English, "en")] {
        for (which, name) in [
            (Page::Miyu, "miyu"),
            (Page::Ask, "ask"),
            (Page::Undo, "undo"),
            (Page::Restore, "restore"),
            (Page::Redo, "redo"),
            (Page::Compact, "compact"),
            (Page::Sandbox, "sandbox"),
        ] {
            let file = dir.join(code).join(format!("{name}.txt"));
            let on_disk = std::fs::read_to_string(&file).expect("有这一页");
            assert_eq!(page(language, which), on_disk, "{}", file.display());
        }
    }
}

/// 在终端里占几列：中日韩的字、全角的标点占两列，别的一列。
fn columns(row: &str) -> usize {
    row.chars()
        .map(|c| match c {
            '\u{1100}'..='\u{115F}'
            | '\u{2E80}'..='\u{A4CF}'
            | '\u{AC00}'..='\u{D7A3}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FE30}'..='\u{FE4F}'
            | '\u{FF00}'..='\u{FF60}'
            | '\u{FFE0}'..='\u{FFE6}' => 2,
            _ => 1,
        })
        .sum()
}

#[test]
fn pages_fit_in_eighty_columns_and_end_with_one_newline() {
    for language in [Language::Chinese, Language::English] {
        for which in [
            Page::Miyu,
            Page::Ask,
            Page::Undo,
            Page::Restore,
            Page::Redo,
            Page::Compact,
            Page::Sandbox,
        ] {
            let text = page(language, which);
            assert!(
                text.ends_with('\n') && !text.ends_with("\n\n"),
                "{language:?} {which:?}"
            );
            for row in text.lines() {
                assert!(columns(row) <= 80, "{language:?} {which:?}: {row}");
                assert!(
                    !row.ends_with(' '),
                    "{language:?} {which:?}: 行尾有空格：{row}"
                );
            }
        }
    }
}
