//! 扩展进程（施工 9-4 上，`docs/blueprint/extensions.md`）：真核心拉起真进程走一遍。扩展是测试用的那个小程序
//! （`miyu-test-extension`），拷一份放在测试程序旁边（包的程序只找主程序旁边的，测试里的主程序就是测试程序自己），参数写它
//! 一步步做什么。等多久、退避多久设短的。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

#[tokio::test]
async fn enabling_starts_it_in_its_own_directory_and_it_shakes_hands_without_a_token() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "echo");
    let args = [
        keep.as_str(),
        "err:hello from echo",
        "hello",
        "call:extension.status",
        "wait",
    ];
    install(&home, "echo", &program.name(), "manual", &steps(&args));
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let off = status(&mut client, "echo").await;
    assert_eq!(
        off,
        json!({"package": "echo", "name": "回声", "start": "manual", "on": false, "state": "off", "failures": 0}),
        "manual 的不用开就不拉起"
    );

    let enabled = call(&mut client, "extension.enable", "echo").await;
    assert_eq!(enabled["result"]["on"], true, "{enabled}");
    let running = until_state(&mut client, "echo", |one| one["state"] == "running").await;
    assert!(running["pid"].as_u64().is_some(), "{running}");
    // 它握完手、调完那一句才算记完：等记下三行。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).lines().count() < 3 {
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let recorded = read(&path);
    let lines: Vec<&str> = recorded.lines().collect();
    let dir = home.root.state().join("packages").join("echo");
    assert_eq!(
        std::fs::canonicalize(lines[0].trim_start_matches("cwd:")).expect("在"),
        std::fs::canonicalize(&dir).expect("建了"),
        "工作目录是它放状态的目录"
    );
    let hello: Value = serde_json::from_str(lines[1]).expect("回应是 JSON");
    assert_eq!(
        hello["result"]["account"], "alice",
        "没出示凭据也握成了：{hello}"
    );
    let refused: Value = serde_json::from_str(lines[2]).expect("回应是 JSON");
    assert_eq!(
        refused["error"]["data"]["reason"], "local_only",
        "扩展不能开关扩展：{refused}"
    );
    assert_eq!(stderr(&home, "echo"), "hello from echo\n", "标准错误进日志");
    assert_eq!(
        read(&home.root.system().join("extensions.json")),
        "{\"version\":1,\"on\":{\"echo\":true}}\n"
    );

    let disabled = call(&mut client, "extension.disable", "echo").await;
    assert_eq!(disabled["result"]["state"], "off", "{disabled}");
    assert_eq!(disabled["result"]["on"], false);
    assert_eq!(
        read(&home.root.system().join("extensions.json")),
        "{\"version\":1,\"on\":{\"echo\":false}}\n"
    );
}

#[tokio::test]
async fn it_runs_with_the_cores_data_root_and_resources() {
    // 施工 O-18：拉起时把 `MIYU_HOME`、`MIYU_RESOURCES` 设成核心手上的数据根、资源目录，不管测试程序自己的环境里是什么。
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "echo");
    let args = [
        keep.as_str(),
        "env:MIYU_HOME",
        "env:MIYU_RESOURCES",
        "hello",
        "wait",
    ];
    install(&home, "echo", &program.name(), "always", &steps(&args));
    let core = core(&home, quick());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).lines().count() < 3 {
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let recorded = read(&path);
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        lines[1],
        format!("MIYU_HOME={}", home.root.path().display())
    );
    assert_eq!(
        lines[2],
        format!("MIYU_RESOURCES={}", default_resources().display())
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn always_ones_start_with_the_core_and_a_new_core_follows_the_switches() {
    let home = Home::new();
    let program = Program::new();
    let waits = steps(&["hello", "wait"]);
    install(&home, "auto", &program.name(), "always", &waits);
    install(&home, "echo", &program.name(), "manual", &waits);
    let first = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&first));
    client.hello().await;
    let auto = until_state(&mut client, "auto", |one| one["state"] == "running").await;
    assert_eq!(auto["on"], true, "always 的不用开：{auto}");
    call(&mut client, "extension.enable", "echo").await;
    call(&mut client, "extension.disable", "auto").await;
    until_state(&mut client, "echo", |one| one["state"] == "running").await;
    drop(client);
    first.stop_extensions().await;

    let second = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&second));
    client.hello().await;
    until_state(&mut client, "echo", |one| one["state"] == "running").await;
    let auto = status(&mut client, "auto").await;
    assert_eq!(
        (&auto["on"], &auto["state"]),
        (&json!(false), &json!("off")),
        "关了的照关着：{auto}"
    );
    second.stop_extensions().await;
    let echo = status(&mut client, "echo").await;
    assert_eq!(echo["state"], "off", "随核心退出：{echo}");
}

#[tokio::test]
async fn an_open_extension_keeps_the_core_from_idling() {
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        "auto",
        &program.name(),
        "always",
        &steps(&["hello", "wait"]),
    );
    let core = core(&home, quick());
    assert!(!core.idle().await, "拉起了就不算空闲");
    {
        let mut client = Client::connect(Arc::clone(&core));
        client.hello().await;
        until_state(&mut client, "auto", |one| one["state"] == "running").await;
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while core.connections() > 1 {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!core.idle().await, "只剩它自己的连接，照样不空闲");
    core.stop_extensions().await;
    assert!(core.idle().await, "停了就空闲");
}

#[tokio::test]
async fn exit_code_one_stops_at_once_with_the_end_of_its_stderr() {
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        "echo",
        &program.name(),
        "manual",
        &steps(&["err:port 6700 is taken", "exit:1"]),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    call(&mut client, "extension.enable", "echo").await;
    let stopped = until_state(&mut client, "echo", |one| one["state"] == "stopped").await;
    assert_eq!(stopped["reason"], "config_error", "{stopped}");
    assert_eq!(stopped["failures"], 1);
    assert_eq!(stopped["stderr"], "port 6700 is taken");
    assert_eq!(stopped["on"], true, "停下了照旧开着");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(stderr(&home, "echo"), "port 6700 is taken\n", "没再拉起");
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while !core.idle().await {
        assert!(tokio::time::Instant::now() < deadline, "停下的不拦着空闲");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn other_exits_back_off_and_five_in_a_row_stop_it() {
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        "echo",
        &program.name(),
        "manual",
        &steps(&["err:up", "hello", "exit:3"]),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    call(&mut client, "extension.enable", "echo").await;
    let stopped = until_state(&mut client, "echo", |one| one["state"] == "stopped").await;
    assert_eq!(stopped["reason"], "failed_repeatedly", "{stopped}");
    assert_eq!(stopped["failures"], 5);
    assert_eq!(stderr(&home, "echo"), "up\n".repeat(5), "拉起了五次");
}

#[tokio::test]
async fn one_that_never_says_hello_fails_too() {
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        "echo",
        &program.name(),
        "manual",
        &steps(&["err:up", "wait"]),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_hello_wait(Duration::from_millis(100)),
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    call(&mut client, "extension.enable", "echo").await;
    let stopped = until_state(&mut client, "echo", |one| one["state"] == "stopped").await;
    assert_eq!(stopped["reason"], "failed_repeatedly", "{stopped}");
    assert_eq!(stderr(&home, "echo"), "up\n".repeat(5));
}

#[tokio::test]
async fn one_that_ignores_the_closed_input_is_killed_after_the_grace() {
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        "echo",
        &program.name(),
        "manual",
        &steps(&["hello", "hang"]),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    call(&mut client, "extension.enable", "echo").await;
    let running = until_state(&mut client, "echo", |one| one["state"] == "running").await;
    let disabled = call(&mut client, "extension.disable", "echo").await;
    assert_eq!(disabled["result"]["state"], "off", "{disabled}");
    #[cfg(unix)]
    {
        let pid = running["pid"].as_u64().expect("有进程号").to_string();
        let alive = std::process::Command::new("kill")
            .args(["-0", &pid])
            .stderr(std::process::Stdio::null())
            .status()
            .expect("跑得起 kill");
        assert!(!alive.success(), "杀掉了");
    }
    #[cfg(not(unix))]
    drop(running);
}

#[tokio::test]
async fn restart_starts_a_new_process_and_the_count_again() {
    let home = Home::new();
    let program = Program::new();
    install(
        &home,
        "echo",
        &program.name(),
        "manual",
        &steps(&["hello", "wait"]),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let off = call(&mut client, "extension.restart", "echo").await;
    assert_eq!(off["error"]["data"]["reason"], "extension_off", "{off}");
    call(&mut client, "extension.enable", "echo").await;
    let first = until_state(&mut client, "echo", |one| one["state"] == "running").await;
    let restarted = call(&mut client, "extension.restart", "echo").await;
    assert_eq!(restarted["result"]["on"], true, "{restarted}");
    let second = until_state(&mut client, "echo", |one| {
        one["state"] == "running" && one["pid"] != first["pid"]
    })
    .await;
    assert_eq!(second["failures"], 0);
    core.stop_extensions().await;
}

#[tokio::test]
async fn what_cannot_start_says_why_and_wrong_packages_are_refused() {
    let home = Home::new();
    install(
        &home,
        "ghost",
        "miyu-no-such-program-anywhere",
        "manual",
        &[],
    );
    home.write(
        "home/alice/packages/future.toml",
        "[package]\nkind = \"process\"\nprotocol = [2, 3]\nname = { en = \"F\" }\n\n[command]\nname = \"future\"\nprogram = \"miyu-no-such-program-anywhere\"\nabout = { en = \"F\" }\n",
    );
    home.write(
        "home/alice/packages/face.toml",
        "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = { en = \"Face\" }\n",
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let ghost = call(&mut client, "extension.enable", "ghost").await;
    assert_eq!(ghost["result"]["state"], "stopped", "{ghost}");
    assert_eq!(ghost["result"]["reason"], "not_installed");
    let future = call(&mut client, "extension.enable", "future").await;
    assert_eq!(future["result"]["reason"], "protocol_mismatch", "{future}");
    let listed = client.call("s", "extension.status", json!({})).await;
    let ids: Vec<&str> = listed["result"]["extensions"]
        .as_array()
        .expect("是数组")
        .iter()
        .filter_map(|one| one["package"].as_str())
        .collect();
    assert!(!ids.contains(&"face"), "界面不列：{ids:?}");
    for (id, reason) in [("face", "not_an_extension"), ("nope", "unknown_package")] {
        let refused = call(&mut client, "extension.enable", id).await;
        assert_eq!(refused["error"]["data"]["reason"], reason, "{refused}");
    }
    let bad = client
        .call(
            "b",
            "extension.enable",
            json!({"package": "ghost", "more": 1}),
        )
        .await;
    assert_eq!(bad["error"]["data"]["reason"], "bad_params", "{bad}");
}
