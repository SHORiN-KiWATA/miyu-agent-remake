//! 印出来的样子（项目主人 2026-09-28 定的）：撤销、恢复一行行对；每种没动的原因、出错；差异、还有几行；执行过命令的
//! 那一句；上色；英文。

use std::path::MAIN_SEPARATOR_STR;

use serde_json::{Value, json};

use super::*;

/// 根目录下面的一处，`parts` 一段段接上：Windows 上得带盘符才算绝对路径。
fn under(parts: &[&str]) -> String {
    let root = PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" });
    parts
        .iter()
        .fold(root, |path, part| path.join(part))
        .to_string_lossy()
        .into_owned()
}

/// 照平台的分隔符写一条相对路径。
fn native(path: &str) -> String {
    path.replace('/', MAIN_SEPARATOR_STR)
}

/// 家目录在根下的 `home/me`，会话的工作目录是 `home/me/proj`。
fn plan(direction: Direction, language: Language) -> UndoPlan {
    UndoPlan {
        direction,
        session: None,
        language,
        home: Some(PathBuf::from(under(&["home", "me"]))),
        color: false,
    }
}

fn printed(result: &Value, plan: &UndoPlan) -> String {
    print::lines(result, plan)
        .iter()
        .map(|line| line.paint(plan.color))
        .collect()
}

/// 定样子时的那个例子：写回的、移回来的、新建的移进回收站、之后又被改过的，执行过两条命令。
fn agreed() -> Value {
    let at = |parts: &[&str]| {
        let mut all = vec!["home", "me", "proj"];
        all.extend(parts);
        under(&all)
    };
    json!({
        "events": [14, 15],
        "cwd": under(&["home", "me", "proj"]),
        "turns": 1,
        "said": "把 README 改成中文",
        "commands": 2,
        "files": [
            {"path": at(&["src", "a.rs"]), "action": "write", "outcome": "restored"},
            {"path": at(&["docs", "old.md"]), "action": "untrash", "outcome": "restored"},
            {"path": at(&["notes", "new.txt"]), "action": "trash", "outcome": "restored"},
            {"path": at(&["src", "b.rs"]), "action": "write", "outcome": "changed",
             "diff": ["@@ -3 +3 @@", "-fn main() {}", "+fn main() { println!(\"hi\"); }"]},
        ],
    })
}

#[test]
fn the_undo_is_the_sample_of_the_drawing() {
    // 蓝图 `cli/undo.md` 的样本（施工 4-9 三补）：和 `docs/designs/samples/cli/undo-text.txt` 逐字节一样；蓝图里的
    // 那一块，门禁和同一份文件比。样本照 Unix 的路径写。
    let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/cli/undo-text.txt");
    let drawn = std::fs::read_to_string(&sample).expect("有样本");
    let screen = printed(&agreed(), &plan(Direction::Undo, Language::Chinese));
    assert_eq!(screen.replace(MAIN_SEPARATOR_STR, "/"), drawn);
}

#[test]
fn an_undo_is_printed_the_way_it_was_agreed() {
    let (a, old, new, b) = (
        native("src/a.rs"),
        native("docs/old.md"),
        native("notes/new.txt"),
        native("src/b.rs"),
    );
    assert_eq!(
        printed(&agreed(), &plan(Direction::Undo, Language::Chinese)),
        format!(
            "· 撤销「把 README 改成中文」这一轮
· 改回 {a}
· 移回 {old}
· 删掉 {new} → 移进了回收站
· 改回 {b} → 没动：之后又被改过
    --- 她改完的
    +++ 现在
    @@ -3 +3 @@
    -fn main() {{}}
    +fn main() {{ println!(\"hi\"); }}
· 这一轮执行过 2 条命令：命令改的文件撤不回
发下一句之前，可以用 miyu restore 恢复。
"
        )
    );
}

#[test]
fn a_restore_says_what_it_brought_back() {
    let mut result = agreed();
    result["commands"] = Value::Null;
    let printed = printed(&result, &plan(Direction::Restore, Language::Chinese));
    assert!(
        printed.starts_with("· 恢复「把 README 改成中文」这一轮\n"),
        "{printed}"
    );
    assert!(printed.contains("    --- 撤销以后的\n"), "{printed}");
    assert!(!printed.contains("命令"), "恢复时不说命令：{printed}");
    assert!(!printed.contains("miyu restore"), "{printed}");
}

#[test]
fn every_reason_it_was_left_alone_is_said() {
    let files: Vec<Value> = [
        ("missing", "文件没了"),
        ("occupied", "原处有了别的"),
        ("gone", "回收站里已经没有了"),
        ("unsaved", "改前的内容当时没存下来"),
        ("unavailable", "回收站收不了"),
    ]
    .iter()
    .map(|(outcome, _)| json!({"path": under(&["w", "a"]), "action": "write", "outcome": outcome}))
    .chain([
        json!({"path": under(&["w", "a"]), "action": "write", "outcome": "failed", "error": "Permission denied"}),
        json!({"path": under(&["w", "a"]), "action": "write", "outcome": "skipped"}),
    ])
    .collect();
    let result = json!({"cwd": under(&["w"]), "turns": 1, "files": files});
    let printed = printed(&result, &plan(Direction::Undo, Language::Chinese));
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(lines[0], "· 撤销最后一轮", "那一轮不是人开的：{printed}");
    assert_eq!(
        &lines[1..8],
        [
            "· 改回 a → 没动：文件没了",
            "· 改回 a → 没动：原处有了别的",
            "· 改回 a → 没动：回收站里已经没有了",
            "· 改回 a → 没动：改前的内容当时没存下来",
            "· 改回 a → 没动：回收站收不了",
            "· 改回 a → 出错：Permission denied",
            "· 改回 a → 没动",
        ]
    );
}

#[test]
fn a_long_diff_says_how_many_lines_are_left() {
    let result = json!({"cwd": under(&["w"]), "turns": 1, "files": [
        {"path": under(&["w", "a"]), "action": "write", "outcome": "changed", "diff": ["@@ -1 +1 @@", "-x", "+y"], "more": 12},
    ]});
    let printed = printed(&result, &plan(Direction::Undo, Language::Chinese));
    assert!(printed.contains("    +y\n    还有 12 行\n"), "{printed}");
}

/// 路径：在会话的工作目录里的写相对的，家目录里的写 `~/…`，别处的写绝对路径。
#[test]
fn paths_are_written_short() {
    let result = json!({"cwd": under(&["home", "me", "proj"]), "turns": 1, "files": [
        {"path": under(&["home", "me", "other", "x.txt"]), "action": "write", "outcome": "restored"},
        {"path": under(&["srv", "y.txt"]), "action": "write", "outcome": "restored"},
    ]});
    let printed = printed(&result, &plan(Direction::Undo, Language::Chinese));
    let home = format!("· 改回 ~{}\n", native("/other/x.txt"));
    assert!(printed.contains(&home), "{printed}");
    let far = format!("· 改回 {}\n", under(&["srv", "y.txt"]));
    assert!(printed.contains(&far), "{printed}");
}

#[test]
fn with_color_what_was_left_alone_is_red_and_added_lines_green() {
    let mut plan = plan(Direction::Undo, Language::Chinese);
    plan.color = true;
    let printed = printed(&agreed(), &plan);
    let left = format!(
        "\x1b[90m· 改回 {} → \x1b[31m没动\x1b[90m：之后又被改过\x1b[0m\n",
        native("src/b.rs")
    );
    assert!(printed.contains(&left), "{printed:?}");
    assert!(
        printed.contains("\x1b[90m    \x1b[31m-fn main() {}\x1b[0m\n"),
        "{printed:?}"
    );
    assert!(
        printed.contains("\x1b[90m    \x1b[32m+fn main() { println!(\"hi\"); }\x1b[0m\n"),
        "{printed:?}"
    );
}

#[test]
fn in_english_too() {
    let printed = printed(&agreed(), &plan(Direction::Undo, Language::English));
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(
        lines[0],
        "· Undid the turn \u{201c}把 README 改成中文\u{201d}"
    );
    assert_eq!(
        lines[3],
        format!("· Removed {} → moved to the trash", native("notes/new.txt"))
    );
    assert_eq!(
        lines[4],
        format!(
            "· Restored {} → left alone: changed since",
            native("src/b.rs")
        )
    );
    assert_eq!(lines[5], "    --- as she left it");
    assert_eq!(
        lines[10],
        "· 2 commands ran: files they changed cannot be undone"
    );
    assert_eq!(
        lines[11],
        "Until you say something else, miyu restore brings it back."
    );
}

#[test]
fn with_color_a_failure_is_red_too() {
    let mut plan = plan(Direction::Undo, Language::Chinese);
    plan.color = true;
    let result = json!({"cwd": under(&["w"]), "turns": 1, "files": [
        {"path": under(&["w", "a"]), "action": "write", "outcome": "failed", "error": "Permission denied"},
    ]});
    let printed = printed(&result, &plan);
    assert!(
        printed.contains("→ \x1b[31m出错\x1b[90m：Permission denied\x1b[0m\n"),
        "{printed:?}"
    );
}

/// 太长的截断：人说的话留前面 40 个字，路径留后面 80 个字（文件名在后面）。
#[test]
fn long_words_and_paths_are_cut() {
    let said = "说".repeat(50);
    let deep: Vec<String> = (0..30).map(|n| format!("dir{n}")).collect();
    let mut parts: Vec<&str> = vec!["w"];
    parts.extend(deep.iter().map(String::as_str));
    parts.push("a.txt");
    let result = json!({"cwd": under(&["w"]), "turns": 1, "said": said, "files": [
        {"path": under(&parts), "action": "write", "outcome": "restored"},
    ]});
    let printed = printed(&result, &plan(Direction::Undo, Language::Chinese));
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(lines[0], format!("· 撤销「{}…」这一轮", "说".repeat(40)));
    let shown = lines[1].strip_prefix("· 改回 …").expect("路径截了前面");
    assert_eq!(shown.chars().count(), 80, "{shown}");
    assert!(shown.ends_with("a.txt"), "{shown}");
}

/// 恢复的是几轮的（一次撤了几轮）：写「起的几轮」。
#[test]
fn several_turns_are_counted() {
    let result = json!({"cwd": under(&["w"]), "turns": 3, "said": "第一句", "files": []});
    let printed = printed(&result, &plan(Direction::Restore, Language::Chinese));
    assert_eq!(printed, "· 恢复「第一句」起的 3 轮\n");
    let printed = printed_as(&result, Direction::Undo);
    assert!(
        printed.starts_with("· 撤销「第一句」起的 3 轮\n"),
        "{printed}"
    );
}

/// 照 `direction` 印，中文，不上色。
fn printed_as(result: &Value, direction: Direction) -> String {
    printed(result, &plan(direction, Language::Chinese))
}

/// 英文分单复数（施工 4-9 再补一）：一条命令、还有一行。
#[test]
fn in_english_one_is_one() {
    let result = json!({"cwd": under(&["w"]), "turns": 1, "said": "hi", "commands": 1, "files": [
        {"path": under(&["w", "a"]), "action": "write", "outcome": "changed",
         "diff": ["@@ -1 +1 @@", "-x"], "more": 1},
    ]});
    let printed = printed(&result, &plan(Direction::Undo, Language::English));
    assert!(printed.contains("    1 more line\n"), "{printed}");
    assert!(
        printed.contains("· 1 command ran: files it changed cannot be undone\n"),
        "{printed}"
    );
}

/// 差异里的制表符照原样留着，别的控制字符换成 `�`（施工 4-9 再补一）。
#[test]
fn tabs_in_a_diff_are_kept() {
    let result = json!({"cwd": under(&["w"]), "turns": 1, "files": [
        {"path": under(&["w", "a"]), "action": "write", "outcome": "changed",
         "diff": ["@@ -1 +1 @@", "-\tif x {", "+\tif y {\u{7}"]},
    ]});
    let printed = printed_as(&result, Direction::Undo);
    assert!(printed.contains("    -\tif x {\n"), "{printed:?}");
    assert!(printed.contains("    +\tif y {\u{fffd}\n"), "{printed:?}");
}

#[test]
fn every_first_line_of_a_restore_names_it_restore() {
    // 施工 4-7 补改名：恢复的第一行照新名字写，英文是 Restored，没有 Redid。
    let english = Language::English;
    assert_eq!(
        english.undo_header(Direction::Restore, Some("hi"), 1),
        "· Restored the turn \u{201c}hi\u{201d}"
    );
    assert_eq!(
        english.undo_header(Direction::Restore, Some("hi"), 3),
        "· Restored 3 turns from \u{201c}hi\u{201d}"
    );
    assert_eq!(
        english.undo_header(Direction::Restore, None, 1),
        "· Restored the undone turn"
    );
    assert_eq!(
        Language::Chinese.undo_header(Direction::Restore, None, 1),
        "· 恢复撤销的那一轮"
    );
}

/// 撤掉了压缩（施工 6-9）：第一行下面、文件的几行上面说一句，撤掉一次、几次都是这一句（2026-09-29 项目主人定）；是 0
/// 的、没有这一格的、恢复的不说。
#[test]
fn an_undone_compaction_is_said_once_below_the_first_line() {
    let result = |compactions: u64| {
        json!({"cwd": under(&["w"]), "turns": 1, "said": "接着来", "compactions": compactions, "files": [
            {"path": under(&["w", "a"]), "action": "write", "outcome": "restored"},
        ]})
    };
    let once = "· 撤销「接着来」这一轮\n· 撤掉了压缩，上下文回到了压缩前\n· 改回 a\n发下一句之前，可以用 miyu restore 恢复。\n";
    assert_eq!(printed_as(&result(1), Direction::Undo), once);
    assert_eq!(
        printed_as(&result(2), Direction::Undo),
        once,
        "几次都是同一句"
    );
    assert!(!printed_as(&result(0), Direction::Undo).contains("压缩"));
    let mut without = result(1);
    without.as_object_mut().map(|map| map.remove("compactions"));
    assert!(!printed_as(&without, Direction::Undo).contains("压缩"));
    assert!(!printed_as(&result(1), Direction::Restore).contains("压缩"));
    let english = printed(&result(3), &plan(Direction::Undo, Language::English));
    assert!(
        english.contains(
            "\n· Undid the compaction; the context is back to how it was before\n· Restored a\n"
        ),
        "{english}"
    );
}
