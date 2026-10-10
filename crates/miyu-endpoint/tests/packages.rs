//! 软件包清单（施工 9-1 上，`docs/blueprint/packages.md`）：真核心走一遍。`package.list` 列出出厂的、管理员家目录里的，
//! 名字、说明照连接的语言挑；写错的、撞了的、协议版本对不上的带代码和给人看的一句；核心起来时读一次；`check` 查清单。
//!
//! 测试用的是仓库的资源目录：出厂的清单会越来越多（终端界面、桥），这里只断言出厂的网页和测试自己放的那几份，家目录里
//! 放的编号、子命令名都避开出厂会有的。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 终端界面的会话 2026-10-07 给的那份草稿：编号、子命令名改成 `term`，出厂以后才有的 `tui` 撞不上它。
const TERM: &str = r#"[package]
kind = "ui"
version = "0.0.1"
protocol = [1, 1]
name = { en = "Terminal interface", zh = "终端界面", ja = "ターミナル画面" }
summary = { en = "Chat with her in the terminal", zh = "在终端里和她对话" }

[command]
name = "term"
program = "miyu-tui"
about = { en = "Open the terminal interface", zh = "打开终端界面", ja = "ターミナル画面を開く" }

[ui]
opens = ["config"]
"#;

/// 管理员（测试里是 alice）家目录里的一份清单。
fn mine(home: &Home, file: &str, text: &str) {
    home.write(&format!("home/alice/packages/{file}"), text);
}

/// 只留这几个编号的，照列出的先后。
fn only(packages: &[Value], ids: &[&str]) -> Vec<Value> {
    packages
        .iter()
        .filter(|package| ids.iter().any(|id| package["package"] == *id))
        .cloned()
        .collect()
}

async fn listed(home: &Home) -> Vec<Value> {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client.call("p1", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .clone()
}

#[tokio::test]
async fn shipped_and_home_packages_are_listed_in_the_connections_language() {
    let home = Home::new();
    mine(&home, "term.toml", TERM);
    let packages = listed(&home).await;
    let state = home.root.path().join("state").join("packages").join("term");
    assert_eq!(
        only(&packages, &["term", "web"]),
        [
            json!({
                "package": "term",
                "layer": "home",
                "kind": "ui",
                "version": "0.0.1",
                "protocol": [1, 1],
                "name": "终端界面",
                "summary": "在终端里和她对话",
                "command": {"name": "term", "program": "miyu-tui", "about": "打开终端界面"},
                "opens": ["config"],
                "state": state.to_string_lossy(),
                "status": "program_missing",
            }),
            json!({
                "package": "web",
                "layer": "shipped",
                "kind": "ui",
                "protocol": [1, 1],
                "name": "网页界面",
                "summary": "在浏览器里和她对话、改配置",
                "command": {"name": "web", "program": "miyu-web", "about": "打开网页界面"},
                "opens": [],
                "pages_dir": "web/pages",
                "state": home.root.path().join("state").join("packages").join("web").to_string_lossy(),
                "status": "program_missing",
            }),
        ],
        "握手说的是中文，照中文挑；没写的格不写"
    );
}

/// 种类多的两种、功能、平台接入、依赖、小程序（施工 F-1，设计 30）：照连接的语言列出来；没写功能的扩展包列出照包算的那一个，
/// 界面、小程序没有 `features`；必需的才有 `required`。
#[tokio::test]
async fn features_connections_and_workers_are_listed() {
    let home = Home::new();
    mine(
        &home,
        "xbase.toml",
        r#"[package]
kind = "builtin"
required = true
protocol = [1, 1]
name = { en = "Base", zh = "基础" }

[features.xfiles]
name = { en = "Files", zh = "文件读写" }
summary = { zh = "读写文件" }
tools = ["read"]

[features.xcmd]
name = { en = "Commands" }

[recommends]
workers = ["xembed"]
"#,
    );
    mine(
        &home,
        "xbridge.toml",
        r#"[package]
kind = "process"
protocol = [1, 1]
name = { en = "Connect X", zh = "接入X" }

[connection]
platform = "x"

[depends]
workers = ["xembed"]
"#,
    );
    mine(
        &home,
        "xembed.toml",
        r#"[package]
kind = "worker"
protocol = [1, 1]
name = { en = "Model" }

[worker]
program = "miyu-xembed"
args = ["serve"]
"#,
    );
    let packages = listed(&home).await;
    let mut got = only(&packages, &["xbase", "xbridge", "xembed"]);
    for package in &mut got {
        package.as_object_mut().unwrap().remove("state");
    }
    assert_eq!(
        got,
        [
            json!({
                "package": "xbase",
                "layer": "home",
                "kind": "builtin",
                "protocol": [1, 1],
                "name": "基础",
                "required": true,
                "features": [
                    {"id": "xfiles", "name": "文件读写", "summary": "读写文件"},
                    {"id": "xcmd", "name": "Commands"},
                ],
                "recommends": {"workers": ["xembed"]},
                "status": "ready",
            }),
            json!({
                "package": "xbridge",
                "layer": "home",
                "kind": "process",
                "protocol": [1, 1],
                "name": "接入X",
                "features": [{"id": "xbridge", "name": "接入X"}],
                "connection": {"platform": "x"},
                "depends": {"workers": ["xembed"]},
                "status": "off",
                "enabled": false,
            }),
            json!({
                "package": "xembed",
                "layer": "home",
                "kind": "worker",
                "protocol": [1, 1],
                "name": "Model",
                "worker": {"program": "miyu-xembed", "args": ["serve"]},
                "status": "program_missing",
            }),
        ]
    );
}

/// 清单是内置包、核心里却没编进它的代码（施工 F-2，设计 30 第二节第 3 条）：照读坏了的清单报 `not_built_in`，别的不动。
#[test]
fn a_builtin_the_core_lacks_is_not_built_in() {
    let home = Home::new();
    mine(
        &home,
        "xghost.toml",
        "[package]\nkind = \"builtin\"\nprotocol = [1, 1]\nname = { en = \"Ghost\" }\n",
    );
    mine(&home, "term.toml", TERM);
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root, &alice());
    miyu_endpoint::packages::compiled(
        &mut found,
        &["basesystem", "memory", "mermaid", "net", "roleplay"],
    );
    let codes: Vec<(&str, Option<&str>)> = found
        .iter()
        .map(|one| {
            let code = match &one.read {
                Ok(_) => None,
                Err(miyu_store::packages::Issue::Wrong(problem)) => Some(problem.code.as_str()),
                Err(miyu_store::packages::Issue::Unreadable(_)) => Some("unreadable"),
            };
            (one.id.as_str(), code)
        })
        .collect();
    assert!(
        codes.contains(&("xghost", Some("not_built_in"))),
        "{codes:?}"
    );
    assert!(codes.contains(&("term", None)), "不是内置的不管");
    assert!(codes.contains(&("basesystem", None)), "编进来了的照常");
    assert!(
        miyu_endpoint::packages::is_installed(&found, "memory"),
        "读成了的内置包算装了"
    );
    assert!(!miyu_endpoint::packages::is_installed(&found, "xghost"));
    assert!(
        !miyu_endpoint::packages::is_installed(&found, "term"),
        "只认内置包"
    );
}

#[tokio::test]
async fn broken_taken_and_mismatched_ones_carry_a_code_and_a_sentence() {
    let home = Home::new();
    mine(&home, "bad.toml", "[package]\nkind = \"daemon\"\n");
    mine(&home, "web.toml", TERM);
    mine(&home, "web2.toml", &TERM.replace("\"term\"", "\"web\""));
    mine(
        &home,
        "later.toml",
        &TERM
            .replace("[1, 1]", "[2, 3]")
            .replace("\"term\"", "\"later\""),
    );
    mine(
        &home,
        "bridge.toml",
        // 子命令不叫 onebot：出厂的桥占着它（施工 O-18）。
        "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = { en = \"Bridge\" }\n\n[command]\nname = \"bridge\"\nprogram = \"miyu-onebot\"\nabout = { en = \"QQ\" }\n\n[process]\nargs = [\"serve\"]\n\n[check]\nargs = [\"check\"]\n",
    );
    let packages = listed(&home).await;
    let by_id = |id: &str| {
        packages
            .iter()
            .filter(|package| package["package"] == id)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        by_id("bad"),
        [json!({
            "package": "bad",
            "layer": "home",
            "code": "bad_kind",
            "line": 2,
            "problem": "package.kind 只能是 ui、process、builtin 或 worker，写的是 daemon",
        })]
    );
    let web = by_id("web");
    assert_eq!(web.len(), 2, "出厂的照常，家目录里同编号的那一份报错");
    assert_eq!(web[1]["code"], "duplicate", "{web:?}");
    assert_eq!(by_id("web2")[0]["code"], "command_taken");
    let later = &by_id("later")[0];
    assert_eq!(later["code"], "protocol_mismatch", "{later}");
    assert_eq!(later["protocol"], json!([2, 3]));
    assert_eq!(later["command"]["name"], "later", "对不上的照样列全");
    let bridge = &by_id("bridge")[0];
    assert_eq!(
        bridge["process"],
        json!({"args": ["serve"], "start": "manual"})
    );
    assert_eq!(bridge["check"], json!({"args": ["check"]}));
}

#[tokio::test]
async fn manifests_are_read_once_when_the_core_starts() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    mine(&home, "term.toml", TERM);
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client.call("p1", "package.list", json!({})).await;
    let ids: Vec<&str> = reply["result"]["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|package| package["package"].as_str())
        .collect();
    assert!(ids.contains(&"web"), "{reply}");
    assert!(!ids.contains(&"term"), "起来以后才放的，重启才认：{reply}");
    assert_eq!(only(&listed(&home).await, &["term"]).len(), 1);
}

#[tokio::test]
async fn check_reads_the_manifests_from_disk() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    mine(&home, "bad.toml", "[package]\nkind = \"daemon\"\n");
    mine(&home, "term.toml", TERM);
    let reply = client.call("c1", "check", json!({})).await;
    let packages: Vec<&Value> = reply["result"]["problems"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|problem| problem["kind"] == "package")
        .collect();
    assert_eq!(
        packages,
        [&json!({
            "kind": "package",
            "file": "home/alice/packages/bad.toml",
            "code": "bad_kind",
            "level": "error",
            "line": 2,
            "message": "package.kind 只能是 ui、process、builtin 或 worker，写的是 daemon",
        })],
        "{reply}"
    );
    let file = home.root.path().join("home/alice/packages/term.toml");
    let reply = client
        .call("c2", "check", json!({"file": file.to_string_lossy()}))
        .await;
    assert_eq!(reply["result"], json!({"problems": []}), "{reply}");
    let file = home.root.path().join("home/alice/packages/bad.toml");
    let reply = client
        .call("c3", "check", json!({"file": file.to_string_lossy()}))
        .await;
    assert_eq!(
        reply["result"]["problems"][0]["code"], "bad_kind",
        "{reply}"
    );
    mine(&home, "notes.txt", "x");
    let file = home.root.path().join("home/alice/packages/notes.txt");
    let reply = client
        .call("c4", "check", json!({"file": file.to_string_lossy()}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_file"), "{reply}");
}
