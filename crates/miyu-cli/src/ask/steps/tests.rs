use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use serde_json::{Value, json};

use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

use super::*;
use crate::ask::{Format, Plan, Target};
use crate::language::Language;

/// 根目录：Windows 上得带盘符才算绝对路径。
fn root() -> PathBuf {
    PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" })
}

/// 在根目录下面：`parts` 一段段接上。
fn under(parts: &[&str]) -> PathBuf {
    parts.iter().fold(root(), |path, part| path.join(part))
}

/// 在工作目录 `/work`、家目录 `/home` 里，照 `language` 说；给人看的字照出厂的。
fn plan(language: Language) -> Plan {
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    Plan {
        text: String::new(),
        target: Target::New,
        format: Format::Text,
        cwd: under(&["work"]).to_string_lossy().into_owned(),
        dirs: Vec::new(),
        files: Vec::new(),
        language,
        human: Human::load(&resources, language.code()).expect("出厂的字读得出来"),
        home: Some(under(&["home"])),
        timeout: None,
        from: None,
        model: None,
    }
}

/// 她调了 `name`，参数原文是 `args`，调用编号 c1；结果的状态是 `status`，说法是 `human`（空的就没有），是工具自己
/// 写的。交回印出来的样子（标题和下面那一块），不上色。
fn line(plan: &Plan, name: &str, args: &str, status: &str, human: Value) -> Option<String> {
    painted(plan, name, args, status, human, false)
}

/// 同 [`line`]，`color` 的上色。
fn painted(
    plan: &Plan,
    name: &str,
    args: &str,
    status: &str,
    human: Value,
    color: bool,
) -> Option<String> {
    let mut steps = Steps::default();
    steps.reply(&json!({"blocks": [
        {"type": "text", "text": "我先看看。"},
        {"type": "tool_call", "call_id": "c1", "name": name, "args": args},
    ]}));
    let mut result = json!({"call_id": "c1", "status": status, "blocks": []});
    if !human.is_null() {
        result["human"] = human;
    }
    steps.result(&result, true, plan, &plan.cwd).map(|drawn| {
        let mut text = drawn.title.paint(color);
        for line in &drawn.below {
            text.push_str(&line.paint(color));
        }
        text
    })
}

/// `read` 读 `file_path`：参数原文。
fn read(file_path: &str) -> String {
    json!({ "file_path": file_path }).to_string()
}

/// 基础系统的说法。
fn said(key: &str, fields: Value) -> Value {
    json!({"key": format!("software/basesystem/{key}"), "fields": fields})
}

/// 内核的说法。
fn core(key: &str) -> Value {
    json!({ "key": format!("core/tool-results/{key}") })
}

#[test]
fn each_status_reads_as_decided() {
    let plan = plan(Language::Chinese);
    let lib = read("src/lib.rs");
    let cases = [
        (
            "ok",
            said("read/lines", json!({"count": "37"})),
            "→ 读取 src/lib.rs · 37 行\n",
        ),
        (
            "error",
            said("common/missing-similar", json!({"similar": "src/main.rs"})),
            "→ 读取 src/lib.rs · 出错：没有这个文件，是不是 src/main.rs\n",
        ),
        (
            "denied",
            core("unattended"),
            "→ 读取 src/lib.rs · 没做：要确认，这里没人能确认\n",
        ),
        (
            "cancelled",
            core("cancelled-before"),
            "→ 读取 src/lib.rs · 打断了，没跑\n",
        ),
        ("skipped", core("skipped"), "→ 读取 src/lib.rs · 跳过了\n"),
    ];
    for (status, human, want) in cases {
        assert_eq!(
            line(&plan, "read", &lib, status, human).as_deref(),
            Some(want),
            "{status}"
        );
    }
    // 没有说法的：第三方的工具，没有显示名，照工具名、状态写。
    let url = json!({"url": "https://example.com"}).to_string();
    let cases = [
        ("ok", "⚙ web_fetch\n"),
        ("error", "⚙ web_fetch · 出错\n"),
        ("denied", "⚙ web_fetch · 没做\n"),
        ("cancelled", "⚙ web_fetch · 打断了\n"),
        ("skipped", "⚙ web_fetch · 跳过了\n"),
        ("later", "⚙ web_fetch\n"),
    ];
    for (status, want) in cases {
        assert_eq!(
            line(&plan, "web_fetch", &url, status, Value::Null).as_deref(),
            Some(want),
            "{status}"
        );
    }
}

#[test]
fn in_english_too() {
    let plan = plan(Language::English);
    let lib = read("src/lib.rs");
    let lines = [
        line(
            &plan,
            "read",
            &lib,
            "ok",
            said("read/lines", json!({"count": "37"})),
        ),
        line(
            &plan,
            "read",
            &lib,
            "error",
            said("common/missing-similar", json!({"similar": "src/main.rs"})),
        ),
        line(
            &plan,
            "grep",
            r#"{"pattern":"fn tools"}"#,
            "denied",
            Value::Null,
        ),
        line(
            &plan,
            "glob",
            r#"{"pattern":"*.rs"}"#,
            "cancelled",
            Value::Null,
        ),
    ];
    assert_eq!(
        lines.map(Option::unwrap_or_default),
        [
            "→ Read src/lib.rs · 37 lines\n",
            "→ Read src/lib.rs · failed: no such file, did you mean src/main.rs\n",
            "✱ Search fn tools · not done\n",
            "✱ Find files *.rs · interrupted\n",
        ]
    );
}

#[test]
fn a_said_that_cannot_be_worded_goes_by_the_status() {
    let plan = plan(Language::Chinese);
    let lib = read("src/lib.rs");
    // 没有这一句的、少了字段的、写法不对的，都当没有说法。
    for human in [
        said("read/nothing-like-this", json!({})),
        said("read/lines", json!({})),
        json!({"key": "software/basesystem/read/lines", "fields": {"count": 37}}),
        json!("read/lines"),
    ] {
        assert_eq!(
            line(&plan, "read", &lib, "error", human.clone()).as_deref(),
            Some("→ 读取 src/lib.rs · 出错\n"),
            "{human}"
        );
        assert_eq!(
            line(&plan, "read", &lib, "ok", human.clone()).as_deref(),
            Some("→ 读取 src/lib.rs\n"),
            "{human}"
        );
    }
}

#[test]
fn titles_are_plain_results_gray_and_failures_red() {
    let plan = plan(Language::Chinese);
    let mian = read("src/mian.rs");
    assert_eq!(
        painted(
            &plan,
            "read",
            &mian,
            "error",
            said("common/missing", json!({})),
            true
        )
        .as_deref(),
        Some("→ 读取 src/mian.rs\x1b[90m · \x1b[31m出错\x1b[90m：没有这个文件\x1b[0m\n")
    );
    assert_eq!(
        painted(&plan, "web_fetch", "{}", "denied", Value::Null, true).as_deref(),
        Some("⚙ web_fetch\x1b[90m · \x1b[31m没做\x1b[0m\n"),
        "没有原因的，红的写完就回到原色"
    );
    assert_eq!(
        painted(
            &plan,
            "read",
            &mian,
            "ok",
            said("read/lines", json!({"count": "1"})),
            true
        )
        .as_deref(),
        Some("→ 读取 src/mian.rs\x1b[90m · 1 行\x1b[0m\n"),
        "做成了的：标题原色，结果灰"
    );
}

#[test]
fn paths_are_written_short() {
    let plan = plan(Language::Chinese);
    let sep = MAIN_SEPARATOR;
    let text = |path: PathBuf| path.to_string_lossy().into_owned();
    let cases = [
        (
            text(under(&["work", "src", "lib.rs"])),
            format!("src{sep}lib.rs"),
        ),
        (text(under(&["work"])), ".".to_string()),
        (
            text(under(&["home", "notes", "plan.md"])),
            format!("~{sep}notes{sep}plan.md"),
        ),
        (text(under(&["home"])), "~".to_string()),
        (
            text(under(&["work-other", "x.rs"])),
            text(under(&["work-other", "x.rs"])),
        ),
        (
            text(under(&["etc", "hosts"])),
            text(under(&["etc", "hosts"])),
        ),
        ("src/x.rs".to_string(), "src/x.rs".to_string()),
        ("~/x.md".to_string(), "~/x.md".to_string()),
        // 太长的留后面：文件名在后面。
        (
            format!("{}/plan.md", "d".repeat(100)),
            format!("…{}/plan.md", "d".repeat(72)),
        ),
        (format!("a/plan.md\n{}", "b"), "a/plan.md…".to_string()),
    ];
    for (path, want) in cases {
        assert_eq!(
            line(&plan, "read", &read(&path), "ok", Value::Null),
            Some(format!("→ 读取 {want}\n")),
            "{path}"
        );
    }
    // 不是路径的参数照原样：模式里写的路径不改。
    let pattern = text(under(&["work", "src"]));
    let args = json!({ "pattern": pattern }).to_string();
    assert_eq!(
        line(&plan, "grep", &args, "ok", Value::Null),
        Some(format!("✱ 搜内容 {pattern}\n"))
    );
}

#[test]
fn a_parameter_called_path_is_a_path_too() {
    let dir = std::env::temp_dir().join(format!("miyu-cli-steps-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("core/human")).expect("建得了目录");
    std::fs::write(
        dir.join("core/human/zh.json"),
        r#"{"tools": {"ls": {"name": "列目录", "subject": "path"}}}"#,
    )
    .expect("写得进");
    let human = Human::load(&ResourceRoot::at(&dir), "zh");
    std::fs::remove_dir_all(&dir).expect("删得掉");
    let plan = Plan {
        human: human.expect("读得出来"),
        ..plan(Language::Chinese)
    };
    let args = json!({"path": under(&["work", "src"]).to_string_lossy()}).to_string();
    assert_eq!(
        line(&plan, "ls", &args, "ok", Value::Null).as_deref(),
        Some("⚙ 列目录 src\n")
    );
}

#[test]
fn values_are_tidied_before_printing() {
    let plan = plan(Language::Chinese);
    let grep = |pattern: &str| json!({ "pattern": pattern }).to_string();
    let long = "x".repeat(100);
    let cases = [
        (grep("fn a\nfn b"), "✱ 搜内容 fn a…\n".to_string()),
        (
            grep("a\u{1b}[31mb\tc"),
            "✱ 搜内容 a\u{FFFD}[31mb\u{FFFD}c\n".to_string(),
        ),
        (grep(&long), format!("✱ 搜内容 {}…\n", "x".repeat(80))),
        (grep(""), "✱ 搜内容\n".to_string()),
        ("[1]".to_string(), "✱ 搜内容\n".to_string()),
        ("{".to_string(), "✱ 搜内容\n".to_string()),
        (r#"{"path":"src"}"#.to_string(), "✱ 搜内容\n".to_string()),
        (r#"{"pattern":7}"#.to_string(), "✱ 搜内容\n".to_string()),
    ];
    for (args, want) in cases {
        assert_eq!(
            line(&plan, "grep", &args, "ok", Value::Null),
            Some(want),
            "{args}"
        );
    }
    // 结果那一句太长的截断；没有显示名的工具名，控制字符换掉、太长的截断。
    let error = "e".repeat(200);
    let printed = line(
        &plan,
        "read",
        &read("a"),
        "error",
        said("common/failed", json!({ "error": error })),
    )
    .expect("有这一行");
    let reason = format!("读不了：{}", "e".repeat(200));
    let kept: String = reason.chars().take(120).collect();
    assert_eq!(printed, format!("→ 读取 a · 出错：{kept}…\n"));
    let name = format!("\u{1b}{}", "n".repeat(49));
    assert_eq!(
        line(&plan, &name, "{}", "ok", Value::Null),
        Some(format!("⚙ \u{FFFD}{}…\n", "n".repeat(39)))
    );
}

#[test]
fn a_result_for_a_call_never_seen_prints_nothing() {
    let plan = plan(Language::Chinese);
    let mut steps = Steps::default();
    steps.reply(
        &json!({"blocks": [{"type": "tool_call", "call_id": "c1", "name": "read", "args": "{}"}]}),
    );
    let other = json!({"call_id": "c9", "status": "ok", "blocks": []});
    assert_eq!(steps.result(&other, true, &plan, &plan.cwd), None);
    assert_eq!(
        Steps::default().result(&json!({"status": "ok"}), true, &plan, &plan.cwd),
        None
    );
}

#[test]
fn a_directory_too_wide_says_where_she_works() {
    let sep = MAIN_SEPARATOR;
    let used = under(&["home", ".miyu", "home", "admin", "workspace"]);
    let used = used.to_string_lossy();
    let home = under(&["home"]).to_string_lossy().into_owned();
    let chinese = Plan {
        cwd: home.clone(),
        ..plan(Language::Chinese)
    };
    assert_eq!(
        moved(&chinese, &used).paint(false),
        format!("· 目录太宽（~），这次在 ~{sep}.miyu{sep}home{sep}admin{sep}workspace 里干活\n")
    );
    let english = Plan {
        cwd: home,
        ..plan(Language::English)
    };
    assert_eq!(
        moved(&english, &used).paint(true),
        format!(
            "\x1b[90m· Working directory too wide (~), using ~{sep}.miyu{sep}home{sep}admin{sep}workspace this time\x1b[0m\n"
        )
    );
}

/// 沙盒用不了那一句（施工 5-4 下）：每种原因照这台机器的系统写成人话；中英两种；不认得的原因照原样写进括号。
#[test]
fn an_unusable_sandbox_is_said_with_why_and_how_to_fix() {
    use miyu_sandbox::Platform;

    let every = [
        Platform::Linux,
        Platform::Macos,
        Platform::Windows,
        Platform::Other,
    ];
    let cases: [(&str, &[Platform], &str, &str); 6] = [
        (
            "helper_missing",
            &every,
            "主程序旁边没有 miyu-sandbox：重装一次 Miyu",
            "miyu-sandbox is missing beside the main program: reinstall Miyu",
        ),
        (
            "helper_failed",
            &every,
            "miyu-sandbox 跑不起来：重装一次 Miyu",
            "miyu-sandbox does not run: reinstall Miyu",
        ),
        (
            "no_mechanism",
            &[Platform::Linux],
            "内核没有能用的 Landlock：要 Linux 5.13 起，启动参数的 lsm= 里开着",
            "the kernel has no usable Landlock: Linux 5.13 or later, enabled in the lsm= boot parameter",
        ),
        (
            "no_mechanism",
            &[Platform::Macos],
            "装不上 Seatbelt 配置，Miyu 可能跑在别的沙盒里",
            "the Seatbelt profile cannot be applied; Miyu may be running inside another sandbox",
        ),
        (
            "no_mechanism",
            &[Platform::Windows],
            "这一版在 Windows 上还不能把命令关进沙盒",
            "this version cannot sandbox commands on Windows yet",
        ),
        (
            "no_mechanism",
            &[Platform::Other],
            "这个系统上没有能用的沙盒",
            "no sandbox is available on this system",
        ),
    ];
    for (reason, platforms, chinese, english) in cases {
        for &platform in platforms {
            assert_eq!(
                Language::Chinese.unsandboxed(reason, platform),
                format!("· 沙盒用不了（{chinese}）：执行命令要你确认，miyu ask 里确认不了"),
                "{reason} {platform:?}"
            );
            assert_eq!(
                Language::English.unsandboxed(reason, platform),
                format!(
                    "· Sandbox unavailable ({english}): commands need your approval, which cannot be given in miyu ask"
                ),
                "{reason} {platform:?}"
            );
        }
    }
    assert_eq!(
        Language::Chinese.unsandboxed("no_user\u{1b}[2J", Platform::Windows),
        "· 沙盒用不了（no_user\u{fffd}[2J）：执行命令要你确认，miyu ask 里确认不了",
        "不认得的原因照原样写，控制字符换掉"
    );
}
