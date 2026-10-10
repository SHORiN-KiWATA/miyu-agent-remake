//! 附件照模型收（蓝图 `tui.md`「输入框」第 12 条「照模型收」）：模型收不了图片的，`Ctrl+V` 贴进来的图收成文件块、提示一句，
//! 发出去写路径；收得了的照旧是附件。终端送来的粘贴（`Ctrl+Shift+V`、拖文件）照原样是字（2026-10-10 项目主人）。
//! 剪贴板用工作目录里的假 `wl-paste`（放在 `PATH` 前面），不碰跑测试那台机器上的剪贴板。

mod support;

use miyu_session::testkit::{Play, Script};
use support::Home;

/// 一家、一个模型，配置里写明它只收文字。
const BLIND: &str = "[providers.fake]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\n\n[providers.fake.models.m]\ninputs = [\"text\"]\n\n[models]\nchat = \"fake/m\"\n";
/// 同一个模型，收图。
const SEEING: &str = "[providers.fake]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\n\n[providers.fake.models.m]\ninputs = [\"text\", \"image\"]\n\n[models]\nchat = \"fake/m\"\n";

/// 在工作目录里放一张图，交回它的路径。
fn picture(home: &Home) -> String {
    let path = home.work.join("晚霞.png");
    std::fs::write(&path, b"\x89PNG\r\n\x1a\n").expect("写得进去");
    path.display().to_string()
}

/// 把路径当一次粘贴送进去（终端拖文件进来、`Ctrl+Shift+V` 都是这样）。
fn drop_in(tui: &mut support::Tui, path: &str) {
    tui.key(format!("\x1b[200~{path}\x1b[201~").as_bytes());
}

/// 一个假的 `wl-paste`：剪贴板里是文件管理器复制的这个文件（`text/uri-list`）。交回要设的 `PATH`。
fn copied(home: &Home, path: &str) -> String {
    let bin = home.work.join("bin");
    std::fs::create_dir_all(&bin).expect("建得了目录");
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in\n  --list-types) printf 'text/uri-list\\ntext/plain\\n' ;;\n  --type) [ \"$2\" = text/uri-list ] && printf 'file://%s\\n' '{path}' ;;\n  *) printf '%s' '{path}' ;;\nesac\n"
    );
    let fake = bin.join("wl-paste");
    std::fs::write(&fake, script).expect("写得进去");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).expect("改得了权限");
    format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// 起界面：剪贴板是上面那个假的。
fn with_clipboard(home: &Home, path: &str) -> support::Tui {
    let path_env = copied(home, path);
    home.tui_with(
        "zh_CN.UTF-8",
        &[
            ("PATH", path_env.as_str()),
            ("WAYLAND_DISPLAY", "miyu-test"),
        ],
    )
}

#[test]
fn a_model_that_cannot_see_images_gets_the_path_instead() {
    let home = Home::with_settings(Script::new([Play::Says("好。")]), BLIND);
    let path = picture(&home);
    let mut tui = with_clipboard(&home, &path);
    tui.wait_for("Tab 切换权限级别");
    tui.pump(std::time::Duration::from_millis(600));
    tui.key(b"\x16");
    tui.wait_for("当前模型不支持图片");
    assert!(
        tui.shows("[晚霞.png]") && !tui.shows("[图片 1]"),
        "{}",
        tui.lines().join("\n")
    );
}

#[test]
fn a_model_that_sees_images_still_gets_an_attachment() {
    let home = Home::with_settings(Script::new([Play::Says("好。")]), SEEING);
    let path = picture(&home);
    let mut tui = with_clipboard(&home, &path);
    tui.wait_for("Tab 切换权限级别");
    tui.pump(std::time::Duration::from_millis(600));
    tui.key(b"\x16");
    tui.wait_for("[图片 1]");
    assert!(!tui.shows("当前模型不支持"));
}

#[test]
fn a_dropped_or_terminal_pasted_path_stays_text() {
    // 2026-10-10 项目主人：「ctrl+shift+V 粘贴文字，ctrl+V 粘贴占位符」。拖文件进来和 Ctrl+Shift+V 哪个终端都是一次粘贴，
    // 分不开，照原样是字。
    let home = Home::with_settings(Script::new([Play::Says("好。")]), SEEING);
    let path = picture(&home);
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("Tab 切换权限级别");
    tui.pump(std::time::Duration::from_millis(600));
    drop_in(&mut tui, &path);
    tui.wait_for("晚霞.png");
    assert!(!tui.shows("[图片 1]"), "{}", tui.lines().join("\n"));
}
