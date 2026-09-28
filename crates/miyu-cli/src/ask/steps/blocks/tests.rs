//! 执行命令、编辑那一块（施工 4-11）：照 `Steps` 走一遍，看标题和下面印的。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

use super::super::Steps;
use crate::ask::{Format, Plan, Target};
use crate::language::Language;

/// 根目录下面的 `parts`：Windows 上得带盘符才算绝对路径。
fn under(parts: &[&str]) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" });
    parts.iter().fold(root, |path, part| path.join(part))
}

/// 在 `/work` 里，照 `language` 说；给人看的字照出厂的。
fn plan(language: Language) -> Plan {
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    Plan {
        text: String::new(),
        target: Target::New,
        format: Format::Text,
        cwd: under(&["work"]).to_string_lossy().into_owned(),
        language,
        human: Human::load(&resources, language.code()).expect("出厂的字读得出来"),
        home: Some(under(&["home"])),
    }
}

/// 她调了 `name`（参数 `args`），结果是 `result`，`by_tool` 是不是工具自己写的。交回印出来的样子，照 `color` 上色；
/// 第二样是标题下面有没有东西。
fn drawn(name: &str, args: &Value, result: &Value, by_tool: bool, color: bool) -> (String, bool) {
    let plan = plan(Language::Chinese);
    let mut steps = Steps::default();
    steps.reply(&json!({"blocks": [
        {"type": "tool_call", "call_id": "c1", "name": name, "args": args.to_string()},
    ]}));
    let mut result = result.clone();
    result["call_id"] = json!("c1");
    let drawn = steps
        .result(&result, by_tool, &plan, &plan.cwd)
        .expect("对得上");
    let mut text = drawn.title.paint(color);
    for line in &drawn.below {
        text.push_str(&line.paint(color));
    }
    (text, !drawn.below.is_empty())
}

/// 执行命令 `command`，结果是这几块文字、这个状态、这句说法。
fn shell(command: &str, texts: &[&str], status: &str, by_tool: bool) -> (String, bool) {
    let blocks: Vec<Value> = texts
        .iter()
        .map(|text| json!({"type": "text", "text": text}))
        .collect();
    let result = json!({"status": status, "blocks": blocks,
        "human": {"key": "software/basesystem/shell/exited", "fields": {"code": "101"}}});
    drawn(
        "shell",
        &json!({ "command": command }),
        &result,
        by_tool,
        false,
    )
}

#[test]
fn a_command_prints_what_she_saw_under_it() {
    // 工具自己写的：标题不写结果那一句，下面照原样印她看到的；几块接起来，末尾的换行不多出一个空行。
    let (text, block) = shell(
        "cargo test 2>&1 | tail -60",
        &["running 2 tests\ntest adds ... FAILED\n", "Exit code 101\n"],
        "error",
        true,
    );
    assert!(block);
    assert_eq!(
        text,
        "$ cargo test 2>&1 | tail -60\nrunning 2 tests\ntest adds ... FAILED\nExit code 101\n"
    );
    // 命令整行照印，不截；几行的，第二行起前面写 `> `。
    let long = format!("echo {}", "x".repeat(200));
    assert_eq!(
        shell(&long, &["x\n"], "ok", true).0,
        format!("$ {long}\nx\n")
    );
    assert_eq!(
        shell("cat <<'EOF'\nhi\nEOF", &["hi"], "ok", true).0,
        "$ cat <<'EOF'\n> hi\n> EOF\nhi\n",
        "末尾没有换行的，补一个"
    );
    // 结果里没有字的：只有标题，照一行印。
    assert_eq!(
        shell("true", &[], "ok", true),
        ("$ true\n".to_string(), false)
    );
    // 开头、末尾的空行不印，中间的、行首的缩进照留；只有空行的，当没有字。
    assert_eq!(
        shell(
            "cargo test",
            &["\n   Compiling x\n\ntest result: ok\n\n \n"],
            "ok",
            true
        )
        .0,
        "$ cargo test\n   Compiling x\n\ntest result: ok\n"
    );
    assert_eq!(
        shell("true", &["\n  \n"], "ok", true),
        ("$ true\n".to_string(), false)
    );
}

#[test]
fn a_command_not_run_says_why_on_its_title() {
    // 不是工具写的（被拒了、没跑就被打断了）：标题后面写结果那一句，下面不印。
    let result = json!({"status": "denied", "blocks": [{"type": "text", "text": "The call was not run."}],
        "human": {"key": "core/tool-results/unattended"}});
    assert_eq!(
        drawn(
            "shell",
            &json!({"command": "rm -rf build"}),
            &result,
            false,
            false
        ),
        (
            "$ rm -rf build · 没做：要确认，这里没人能确认\n".to_string(),
            false
        )
    );
    // 几行的命令，续行照样在下面。
    let (text, block) = drawn("shell", &json!({"command": "a\nb"}), &result, false, false);
    assert_eq!(text, "$ a · 没做：要确认，这里没人能确认\n> b\n");
    assert!(block);
    // 没有命令的：只写符号。
    assert_eq!(
        drawn("shell", &json!({}), &result, false, false).0,
        "$ · 没做：要确认，这里没人能确认\n"
    );
}

#[test]
fn a_failed_command_has_a_red_prompt() {
    let result = json!({"status": "error", "blocks": [{"type": "text", "text": "boom\n"}]});
    assert_eq!(
        drawn("shell", &json!({"command": "false"}), &result, true, true).0,
        "\x1b[31m$\x1b[0m false\x1b[0m\nboom\n"
    );
    let result = json!({"status": "ok", "blocks": [{"type": "text", "text": "fine\n"}]});
    assert_eq!(
        drawn("shell", &json!({"command": "true"}), &result, true, true).0,
        "$ true\nfine\n",
        "做成了的，整块原色"
    );
}

#[test]
fn control_sequences_in_the_output_are_taken_out() {
    // 改颜色的、改标题的整段去掉；别的控制字符换成 `�`，制表符照留；命令里的控制字符也换掉。
    let output =
        "\x1b[31mred\x1b[0m\tok\n\x1b]0;title\x07after\n\x1b]52;c;AAAA\x1b\\x\n\x07bell\x1bcx\n";
    assert_eq!(
        shell("ls\x1b[2J", &[output], "ok", true).0,
        "$ ls\u{FFFD}[2J\nred\tok\nafter\nx\n\u{FFFD}bell\u{FFFD}cx\n"
    );
    // 没收尾的 OSC 去到行尾；CSI 写坏了的，只去掉认得出的那一截。
    assert_eq!(
        shell("ls", &["a\x1b]0;never ends\nb\x1b[1;中\n"], "ok", true).0,
        "$ ls\na\nb中\n"
    );
}

/// 编辑 `a.rs`：参数里的几处，结果照 `status`。
fn edit(edits: &Value, status: &str, color: bool) -> (String, bool) {
    let result = json!({"status": status, "blocks": [],
        "human": {"key": "software/basesystem/edit/edited", "fields": {"count": "2"}}});
    let args = json!({"file_path": "a.rs", "edits": edits});
    drawn("edit", &args, &result, true, color)
}

#[test]
fn an_edit_prints_what_went_out_and_what_came_in() {
    let edits = json!([
        {"old_string": "a - b\n", "new_string": "a + b\n"},
        {"old_string": "x\r\ny", "new_string": "", "replace_all": true},
    ]);
    let (text, block) = edit(&edits, "ok", false);
    assert!(block);
    assert_eq!(
        text, "← 编辑 a.rs · 改了 2 处\n-a - b\n+a + b\n…\n-x\n-y\n",
        "末尾的换行不多出空行，\\r\\n 当一个换行，改成空的没有 + 行"
    );
    assert_eq!(
        edit(&edits, "ok", true).0,
        "← 编辑 a.rs\x1b[90m · 改了 2 处\x1b[0m\n\x1b[31m-a - b\x1b[0m\n\x1b[32m+a + b\x1b[0m\n\x1b[90m…\x1b[0m\n\x1b[31m-x\x1b[0m\n\x1b[31m-y\x1b[0m\n"
    );
    // 制表符照留，别的控制字符换掉。
    let edits = json!([{"old_string": "\tfn a()\x1b[0m", "new_string": "\tfn b()"}]);
    assert_eq!(
        edit(&edits, "ok", false).0,
        "← 编辑 a.rs · 改了 2 处\n-\tfn a()\u{FFFD}[0m\n+\tfn b()\n"
    );
}

#[test]
fn an_edit_that_cannot_be_read_or_did_not_happen_prints_only_its_title() {
    // 读不出的那一处不印；一处都印不出来的，只有标题。
    let edits = json!([
        {"old_string": 1, "new_string": "x"},
        {"old_string": "a", "new_string": "b"},
        {"new_string": "c"},
    ]);
    assert_eq!(
        edit(&edits, "ok", false).0,
        "← 编辑 a.rs · 改了 2 处\n-a\n+b\n",
        "印得出的只有一处：前面不写 …"
    );
    for edits in [json!("a"), json!([{"old_string": null, "new_string": "b"}])] {
        assert_eq!(
            edit(&edits, "ok", false),
            ("← 编辑 a.rs · 改了 2 处\n".to_string(), false),
            "{edits}"
        );
    }
    // 没改成的：下面不印。
    let edits = json!([{"old_string": "a", "new_string": "b"}]);
    for status in ["error", "denied", "cancelled"] {
        assert!(!edit(&edits, status, false).1, "{status}");
    }
}
