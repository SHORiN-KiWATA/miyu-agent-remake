//! 装卸当场生效的扩展那一半（施工 F-5 下，`docs/blueprint/packages.md`「装卸」，设计 30 第九节）：真核心、真的测试扩展走一遍。
//! 装上一个开着就拉起的扩展，当场拉起、它登记的工具进目录；卸掉，当场停下、`extension.status` 不列它，它的工具出目录、
//! 用过它的会话照旧留着、调到时报「已卸载」；升级了的停下再拉起，装别的包时清单没变的不动。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 测试扩展登记的一件工具。
fn tools() -> Value {
    json!({"tools": [{"name": "echo_back", "description": "The echo_back tool.",
                      "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]})
}

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 第 `n` 次请求的工具面。
fn face(script: &Script, n: usize) -> Vec<String> {
    script.requests()[n]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}

/// 会话最后一条工具结果的字。
fn last_result(home: &Home, session: &str) -> Option<String> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .next_back()
}

#[tokio::test]
async fn an_installed_extension_starts_and_a_removed_one_stops() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "xbridge");
    // 先装到家目录里拿到清单的写法，再挪到工作目录当要装的那一份。
    install(
        &home,
        "xbridge",
        &program.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{}", tools()),
            "serve",
            "err:stopped",
        ]),
    );
    let written = home.root.path().join("home/alice/packages/xbridge.toml");
    let source = home.work.join("xbridge.toml");
    std::fs::rename(&written, &source).expect("挪得动");
    let script = Script::new([
        Play::calls(&[("echo_back", "{}")]),
        Play::Says("嗯。"),
        Play::calls(&[("echo_back", "{}")]),
        Play::Says("没了。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let reply = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reply["result"]["package"], "xbridge", "{reply}");
    until_state(&mut client, "xbridge", |entry| entry["state"] == "running").await;
    lines(&path, 3).await;

    let session = client.create("c1", "~").await;
    client.say("s1", &session, "调一下").await;
    home.until_turns(&session, 1).await;
    assert_eq!(face(&script, 0), ["echo_back"], "装上就有它的工具");
    assert_eq!(
        last_result(&home, &session).as_deref(),
        Some("served echo_back")
    );

    let reply = client
        .call("r1", "package.remove", json!({"package": "xbridge"}))
        .await;
    assert_eq!(reply["result"]["removed"], true, "{reply}");
    until("卸掉的停下", || {
        stderr(&home, "xbridge").contains("stopped")
    })
    .await;
    let listed = client.call("s", "extension.status", json!({})).await;
    assert!(
        listed["result"]["extensions"]
            .as_array()
            .expect("有")
            .iter()
            .all(|one| one["package"] != "xbridge"),
        "卸掉的不列：{listed}"
    );
    client.say("s2", &session, "再调一下").await;
    home.until_turns(&session, 2).await;
    assert_eq!(face(&script, 2), ["echo_back"], "用过它的会话照旧留着");
    assert_eq!(
        last_result(&home, &session).as_deref(),
        Some("The tool \"echo_back\" was uninstalled.\n")
    );
    core.stop_extensions().await;
}

/// 升级一个在跑的扩展（施工 F-5 下）：清单变了，先停下再照开关拉起，新起来的那一个又记一遍。
#[tokio::test]
async fn an_upgraded_extension_restarts() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "xup");
    let written = home.root.path().join("home/alice/packages/xup.toml");
    let source = home.work.join("xup.toml");
    install(
        &home,
        "xup",
        &program.name(),
        "always",
        &steps(&[&step, "hello", "serve"]),
    );
    std::fs::rename(&written, &source).expect("挪得动");
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let reply = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reply["result"]["package"], "xup", "{reply}");
    lines(&path, 2).await;
    let running = until_state(&mut client, "xup", |entry| entry["state"] == "running").await;
    install(&home, "xoff", &program.name(), "manual", &steps(&["serve"]));
    let other = home.work.join("xoff.toml");
    std::fs::rename(
        home.root.path().join("home/alice/packages/xoff.toml"),
        &other,
    )
    .expect("挪得动");
    let reply = client
        .call("i0", "package.install", json!({"path": other}))
        .await;
    assert_eq!(reply["result"]["package"], "xoff", "{reply}");
    let same = status(&mut client, "xup").await;
    assert_eq!(same["pid"], running["pid"], "清单没变的不重起：{same}");
    assert_eq!(read(&path).lines().count(), 2, "{}", read(&path));
    install(
        &home,
        "xup",
        &program.name(),
        "always",
        &steps(&[&step, "hello", "err:v2", "serve"]),
    );
    std::fs::rename(&written, &source).expect("挪得动");
    let reply = client
        .call("i2", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reply["result"]["package"], "xup", "{reply}");
    lines(&path, 4).await;
    until_state(&mut client, "xup", |entry| entry["state"] == "running").await;
    core.stop_extensions().await;
}

/// 卸包先停再删（施工 F-5 补）：扩展退出的时候，它的清单还在盘上。
#[tokio::test]
async fn a_removed_extension_stops_before_its_files_go() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "xgone");
    let written = home.root.path().join("home/alice/packages/xgone.toml");
    let check = format!("exists:{}", written.display());
    install(
        &home,
        "xgone",
        &program.name(),
        "always",
        &steps(&[&step, "hello", "serve", &check]),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    until_state(&mut client, "xgone", |entry| entry["state"] == "running").await;
    let reply = client
        .call("r1", "package.remove", json!({"package": "xgone"}))
        .await;
    assert_eq!(reply["result"]["removed"], true, "{reply}");
    assert!(!written.exists(), "删掉了");
    lines(&path, 3).await;
    assert_eq!(
        read(&path).lines().last(),
        Some("exists:true"),
        "停下的时候清单还在：{}",
        read(&path)
    );
    core.stop_extensions().await;
}

/// 换不成、删不成的照原来的清单换回来（施工 F-5 补）：先停下的扩展重新拉起。只在 Unix 上造得出换不成（目录只读）。
#[cfg(unix)]
#[tokio::test]
async fn a_failed_upgrade_or_removal_brings_the_extension_back() {
    use std::os::unix::fs::PermissionsExt;

    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "xstay");
    install(
        &home,
        "xstay",
        &program.name(),
        "always",
        &steps(&[&step, "hello", "serve"]),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    until_state(&mut client, "xstay", |entry| entry["state"] == "running").await;
    lines(&path, 2).await;
    let dir = home.root.path().join("home/alice/packages");
    let source = home.work.join("xstay.toml");
    std::fs::copy(dir.join("xstay.toml"), &source).expect("拷得了");
    let set = |mode| std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode));
    set(0o555).expect("改得了权限");
    let upgraded = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    let removed = client
        .call("r1", "package.remove", json!({"package": "xstay"}))
        .await;
    set(0o755).expect("改得回权限");
    assert_eq!(reason(&upgraded), Some("internal_error"), "{upgraded}");
    assert_eq!(reason(&removed), Some("internal_error"), "{removed}");
    lines(&path, 6).await;
    until_state(&mut client, "xstay", |entry| entry["state"] == "running").await;
    core.stop_extensions().await;
}

/// 升级成撞了别的包的（施工 F-5 补）：原来的那一份放回去，先停下的那一个照原来的清单重新拉起。
#[tokio::test]
async fn an_invalid_upgrade_brings_the_old_extension_back() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "xbad");
    install(
        &home,
        "xbad",
        &program.name(),
        "always",
        &steps(&[&step, "hello", "serve"]),
    );
    let written = home.root.path().join("home/alice/packages/xbad.toml");
    let good = std::fs::read_to_string(&written).expect("读得到");
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    until_state(&mut client, "xbad", |entry| entry["state"] == "running").await;
    lines(&path, 2).await;
    // 子命令名撞了出厂的网页。
    let source = home.work.join("xbad.toml");
    std::fs::write(&source, good.replace("name = \"xbad\"", "name = \"web\"")).expect("写得进");
    let reply = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reason(&reply), Some("package_invalid"), "{reply}");
    assert_eq!(
        std::fs::read_to_string(&written).expect("在"),
        good,
        "原来的放回去了"
    );
    lines(&path, 4).await;
    until_state(&mut client, "xbad", |entry| entry["state"] == "running").await;
    core.stop_extensions().await;
}
