//! 打开默认的界面（施工 9-3）：照清单定怎么开、没装的怎么说。真的连核心、换成程序在 `crates/miyu/tests/heads.rs` 照真
//! 二进制、在伪终端里走一遍。

use std::path::PathBuf;

use miyu_store::packages::Layer;

use super::*;

/// 一份界面包的清单：子命令跑 `program`，认 `opens` 那几页。
fn ui(id: &str, program: &str, opens: &[&str]) -> Found {
    let opens: Vec<String> = opens.iter().map(|page| format!("{page:?}")).collect();
    let text = format!(
        "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = {{ en = \"U\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"U\" }}\n\n[ui]\nopens = [{}]\n",
        opens.join(", ")
    );
    Found {
        id: id.to_string(),
        layer: Layer::Home,
        path: PathBuf::from(format!("/data/home/admin/packages/{id}/package.toml")),
        read: Ok(miyu_config::package::read(&text).expect("清单合写法")),
    }
}

/// 主程序在一个临时目录里，旁边放着 `programs` 这几个程序（空文件就够：只看在不在）。
fn beside(name: &str, programs: &[&str]) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("miyu-cli-head-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了");
    let main = dir.join("miyu");
    std::fs::write(&main, "").expect("写得进");
    for program in programs {
        std::fs::write(
            dir.join(format!("{program}{}", std::env::consts::EXE_SUFFIX)),
            "",
        )
        .expect("写得进");
    }
    (dir, main)
}

#[test]
fn the_head_runs_with_no_arguments_or_with_the_page_it_opens() {
    let (dir, main) = beside("run", &["miyu-tui"]);
    let found = [ui("tui", "miyu-tui", &["config"])];
    let program = std::fs::canonicalize(&dir)
        .unwrap()
        .join(format!("miyu-tui{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(
        plan("tui", None, &found, &main),
        Plan::Run(program.clone(), Vec::new())
    );
    assert_eq!(
        plan("tui", Some("config"), &found, &main),
        Plan::Run(
            program,
            vec![OsString::from("--page"), OsString::from("config")]
        )
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

#[test]
fn a_page_the_head_does_not_open_and_a_missing_head_are_told_apart() {
    let (dir, main) = beside("page", &["miyu-tui", "miyu-web"]);
    let found = [
        ui("tui", "miyu-tui", &[]),
        ui("gone", "miyu-gone", &["config"]),
    ];
    assert_eq!(plan("tui", Some("config"), &found, &main), Plan::NoPage);
    assert_eq!(
        plan("gone", None, &found, &main),
        Plan::NoProgram("miyu-gone".to_string()),
        "有清单、程序不在旁边"
    );
    assert_eq!(
        plan("other", None, &found, &main),
        Plan::Missing,
        "没有清单"
    );
    let mut bridge = ui("bridge", "miyu-web", &[]);
    if let Ok(manifest) = bridge.read.as_mut() {
        manifest.kind = PackageKind::Process;
    }
    assert_eq!(
        plan("bridge", None, &[bridge], &main),
        Plan::Missing,
        "不是界面"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

#[test]
fn a_missing_head_lists_the_installed_ones() {
    let (dir, main) = beside("installed", &["miyu-web", "miyu-gui"]);
    let found = [
        ui("web", "miyu-web", &[]),
        ui("tui", "miyu-tui", &["config"]),
        ui("gui", "miyu-gui", &[]),
    ];
    assert_eq!(
        installed(&found, &main),
        ["web", "gui"],
        "只有清单、程序不在的不算装了"
    );
    assert_eq!(
        unavailable("nope", None, &installed(&found, &main), Language::Chinese),
        "没装 nope 这个界面（ui.head 指着它）。装上它的软件包，或者 miyu config set ui.head <编号> 换成装了的界面：web、gui。"
    );
    assert!(
        unavailable("nope", None, &[], Language::English).contains("no interface is installed yet")
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

#[test]
fn a_head_whose_program_is_missing_says_so() {
    let installed = ["web".to_string()];
    assert_eq!(
        unavailable("tui", Some("miyu-tui"), &installed, Language::Chinese),
        "tui 这个界面的程序 miyu-tui 不在 miyu 旁边（ui.head 指着它）。把它放到 miyu 旁边，或者 miyu config set ui.head <编号> 换成装了的界面：web。"
    );
    assert_eq!(
        unavailable("tui", Some("miyu-tui"), &installed, Language::English),
        "The tui interface's program miyu-tui is not next to miyu (ui.head names it). Put it next to miyu, or switch with miyu config set ui.head <id> to one that is installed: web."
    );
}
