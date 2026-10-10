//! 配置页改人格（蓝图 `tui.md`「配置页」第 36、37、41 条，核心 P-3）：`a` 填名字就建；人格提示词交给 `$EDITOR`，带版本防覆盖；
//! 示范对话一对一对地加；`d` 删除。编辑器用一个小脚本：把固定的字写进交给它的文件。

mod support;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use miyu_session::testkit::{Play, Script};
use support::{Home, Tui};

/// 新建的第一个人格的目录（编号核心起：`persona-1`）。
fn dir(home: &Home) -> PathBuf {
    home.root().join("home/alice/personas/persona-1")
}

/// 等 `check` 成立，最多 [`support::WAIT`]；等不到的把屏幕打出来。
fn until(tui: &mut Tui, what: &str, check: impl Fn() -> bool) {
    let end = Instant::now() + support::WAIT;
    while !check() {
        assert!(
            Instant::now() < end,
            "等不到{what}，屏幕是：\n{}",
            tui.lines().join("\n")
        );
        tui.pump(Duration::from_millis(100));
    }
}

/// 一个当编辑器用的脚本：把 `text` 写进交给它的文件；`also` 不空的另外写一份（装作别处先改了）。
fn editor(work: &Path, text: &str, also: Option<&Path>) -> String {
    let script = work.join("fake-editor.sh");
    let mut body = format!("#!/bin/sh\nprintf '%s\\n' '{text}' > \"$1\"\n");
    if let Some(also) = also {
        let parent = also.parent().expect("有上一层").display();
        body.push_str(&format!(
            "mkdir -p '{parent}'\nprintf '别处写的\\n' > '{}'\n",
            also.display()
        ));
    }
    std::fs::write(&script, body).expect("写得进去");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("改得了权限");
    }
    script.display().to_string()
}

/// 起在配置页，挪到「人格」进去（主菜单第三项：「权限」2026-10-10 并进了「通用」），`a` 建一个「我的人格」：建好开它的窗，光标
/// 停在人格提示词。
fn made(home: &Home, editor: &str) -> Tui {
    let env = [("EDITOR", editor), ("VISUAL", editor)];
    let mut tui = home.tui_args_with("zh_CN.UTF-8", &["config"], &env);
    tui.wait_for("人格和记忆");
    for _ in 0..2 {
        tui.key(b"j");
    }
    tui.key(b"\r");
    // 出厂不带人格（核心 P-4 下）：列表是空的。
    tui.wait_for("暂无人格");
    tui.key(b"a");
    tui.wait_for("新人格名称");
    tui.type_text("我的人格");
    tui.key(b"\r");
    let toml = dir(home).join("persona.toml");
    until(&mut tui, "建好的人格", || {
        std::fs::read_to_string(&toml).is_ok_and(|t| t.contains("我的人格"))
    });
    tui.wait_for("人设提醒短语");
    assert!(
        !tui.shows("persona-1"),
        "编号不出现：\n{}",
        tui.lines().join("\n")
    );
    tui
}

#[test]
fn a_new_persona_gets_a_prompt_from_the_editor_a_dialog_round_and_is_deleted() {
    let home = Home::new(Script::new([Play::Says("好。")]));
    let script = editor(&home.work, "你是一个测试人格。", None);
    let mut tui = made(&home, &script);
    // 光标停在人格提示词：交给编辑器，写回去存好。
    tui.key(b"\r");
    let prompt = dir(&home).join("prompts/persona.md");
    until(&mut tui, "存进去的人格提示词", || {
        std::fs::read_to_string(&prompt).is_ok_and(|t| t.contains("你是一个测试人格。"))
    });
    // 下一行示范对话：开列表，a 加一轮，两格都写了才存。
    tui.key(b"j");
    tui.key(b"\r");
    tui.wait_for("暂无示范对话");
    tui.key(b"a");
    tui.wait_for("添加");
    // 在「你问的」写好回车跳到「AI回的」，写好回车存（2026-10-08 项目主人：不用 Tab 换格）。
    tui.type_text("在吗");
    tui.key(b"\r");
    tui.type_text("在");
    tui.key(b"\r");
    let examples = dir(&home).join("prompts/examples.md");
    until(&mut tui, "存进去的示范对话", || {
        std::fs::read_to_string(&examples)
            .is_ok_and(|t| t.contains("user: 在吗") && t.contains("assistant: 在"))
    });
    tui.key(b"\x1b");
    tui.wait_for("1 轮");
    // 删除：问一句，确定了目录挪走，写「已删除」。
    tui.key(b"d");
    tui.wait_for("删除人格");
    tui.key(b"h");
    tui.key(b"\r");
    until(&mut tui, "挪走的目录", || !dir(&home).exists());
    tui.wait_for("已删除");
}

#[test]
fn a_prompt_changed_elsewhere_meanwhile_is_not_overwritten() {
    let home = Home::new(Script::new([Play::Says("好。")]));
    let elsewhere = dir(&home).join("prompts/persona.md");
    let script = editor(&home.work, "终端写的", Some(&elsewhere));
    let mut tui = made(&home, &script);
    tui.key(b"\r");
    tui.wait_for("已在别处修改");
    let text = std::fs::read_to_string(&elsewhere).unwrap_or_default();
    assert!(
        text.contains("别处写的") && !text.contains("终端写的"),
        "{text}"
    );
}
