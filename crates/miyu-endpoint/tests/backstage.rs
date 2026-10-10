//! 软件后台（施工 F-6 中，`docs/blueprint/package-pages.md`「`package.file`」「`package.methods`、`package.call`、`method.call`」）：
//! 真核心走一遍。后台页的文件放在测试自己装的扩展的包目录里；方法由测试用的扩展登记、答。

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::support::extensions::{Program, core, install, quick, until_state};
use crate::support::*;

/// 测试用的扩展登记的方法：`slow` 的时限一秒。
const METHODS: &str = r#"ask:package.methods:{"methods":[{"name":"echo"},{"name":"boom"},{"name":"slow","timeout_ms":1000}]}"#;

/// 一份写了后台页的清单接在 `install` 写的后面。
fn with_page(home: &Home, id: &str) {
    let path = home
        .root
        .path()
        .join(format!("home/alice/packages/{id}.toml"));
    let text = std::fs::read_to_string(&path).expect("装好了");
    std::fs::write(&path, format!("{text}\n[page]\ndir = \"page\"\n")).expect("写得进");
}

async fn file(client: &mut Client, package: &str, path: &str, offset: u64) -> Value {
    client
        .call(
            "f",
            "package.file",
            json!({"package": package, "path": path, "offset": offset}),
        )
        .await
}

fn bytes(reply: &Value) -> Vec<u8> {
    STANDARD
        .decode(
            reply["result"]["data"]
                .as_str()
                .unwrap_or_else(|| panic!("{reply}")),
        )
        .expect("是 base64")
}

#[tokio::test]
async fn page_files_are_read_inside_the_page_only() {
    let home = Home::new();
    let program = Program::new();
    install(&home, "relay", &program.name(), "manual", &[]);
    with_page(&home, "relay");
    home.write("home/alice/packages/relay/page/index.html", "<p>relay</p>");
    home.write("home/alice/packages/relay/page/assets/app.js", "go()");
    home.write("home/alice/packages/relay/secret.txt", "not for the page");
    let big = "x".repeat(600 * 1024);
    home.write("home/alice/packages/relay/page/big.txt", &big);
    install(&home, "bare", &program.name(), "manual", &[]);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let index = file(&mut client, "relay", "", 0).await;
    assert_eq!(bytes(&index), b"<p>relay</p>", "{index}");
    assert_eq!(
        (
            index["result"]["size"].clone(),
            index["result"]["eof"].clone()
        ),
        (json!(12), json!(true))
    );
    assert_eq!(
        bytes(&file(&mut client, "relay", "assets/app.js", 0).await),
        b"go()"
    );
    assert_eq!(
        bytes(&file(&mut client, "relay", "assets/app.js", 2).await),
        b"()"
    );

    let first = file(&mut client, "relay", "big.txt", 0).await;
    assert_eq!(bytes(&first).len(), 512 * 1024, "一次最多 512 KiB");
    assert_eq!(first["result"]["eof"], false);
    let rest = file(&mut client, "relay", "big.txt", 512 * 1024).await;
    assert_eq!(bytes(&rest).len(), 88 * 1024);
    assert_eq!(rest["result"]["eof"], true);

    for (package, path, reason) in [
        ("relay", "../secret.txt", "bad_params"),
        ("relay", "/etc/hostname", "bad_params"),
        ("relay", "assets", "not_found"),
        ("relay", "none.js", "not_found"),
        ("bare", "", "no_page"),
        ("nothing", "", "unknown_package"),
    ] {
        let reply = file(&mut client, package, path, 0).await;
        assert_eq!(
            reply["error"]["data"]["reason"], reason,
            "{package} {path}：{reply}"
        );
    }
    core.stop_extensions().await;
}

/// 指到页面目录外面的链接不算（只在 Unix 上建得了链接不要权限）。
#[cfg(unix)]
#[tokio::test]
async fn a_link_out_of_the_page_is_not_followed() {
    let home = Home::new();
    let program = Program::new();
    install(&home, "relay", &program.name(), "manual", &[]);
    with_page(&home, "relay");
    home.write("home/alice/packages/relay/page/index.html", "<p>relay</p>");
    home.write("home/alice/packages/relay/secret.txt", "not for the page");
    let dir = home.root.path().join("home/alice/packages/relay");
    std::os::unix::fs::symlink(dir.join("secret.txt"), dir.join("page/leak.txt"))
        .expect("建得了链接");
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let reply = file(&mut client, "relay", "leak.txt", 0).await;
    assert_eq!(reply["error"]["data"]["reason"], "not_found", "{reply}");
    core.stop_extensions().await;
}

async fn call(client: &mut Client, package: &str, method: &str) -> Value {
    client
        .call(
            "c",
            "package.call",
            json!({"package": package, "method": method, "params": {"n": 1}}),
        )
        .await
}

#[tokio::test]
async fn a_page_calls_the_methods_its_program_registered() {
    let home = Home::new();
    let program = Program::new();
    let (record, keep) = crate::support::extensions::record(&home, "relay");
    let args: Vec<String> = [keep.as_str(), "hello", METHODS, "serve"]
        .iter()
        .map(ToString::to_string)
        .collect();
    install(&home, "relay", &program.name(), "manual", &args);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let before = call(&mut client, "relay", "echo").await;
    assert_eq!(
        before["error"]["data"]["reason"], "program_not_running",
        "{before}"
    );

    let on = client
        .call("e", "package.enable", json!({"package": "relay"}))
        .await;
    assert!(on.get("error").is_none(), "{on}");
    until_state(&mut client, "relay", |one| one["state"] == "running").await;
    let registered = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if std::fs::read_to_string(&record)
                .unwrap_or_default()
                .contains(r#""methods":3"#)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(registered.is_ok(), "登记了三个方法");

    let echo = call(&mut client, "relay", "echo").await;
    assert_eq!(echo["result"], json!({"answered": "echo"}), "{echo}");
    let seen = std::fs::read_to_string(&record).expect("记了");
    assert!(
        seen.contains(r#""method":"method.call","params":{"method":"echo","params":{"n":1}}"#),
        "转过去的照原样：{seen}"
    );

    let boom = call(&mut client, "relay", "boom").await;
    assert_eq!(
        (
            boom["error"]["data"]["reason"].clone(),
            boom["error"]["data"]["message"].clone(),
            boom["error"]["data"]["code"].clone(),
        ),
        (json!("method_failed"), json!("boom"), json!(-32001)),
        "{boom}"
    );
    let slow = call(&mut client, "relay", "slow").await;
    assert_eq!(slow["error"]["data"]["reason"], "method_timeout", "{slow}");
    let missing = call(&mut client, "relay", "nope").await;
    assert_eq!(
        (
            missing["error"]["data"]["reason"].clone(),
            missing["error"]["data"]["method"].clone()
        ),
        (json!("unregistered"), json!("nope")),
        "{missing}"
    );
    let unknown = call(&mut client, "nothing", "echo").await;
    assert_eq!(
        unknown["error"]["data"]["reason"], "unknown_package",
        "{unknown}"
    );

    let off = client
        .call("d", "package.disable", json!({"package": "relay"}))
        .await;
    assert!(off.get("error").is_none(), "{off}");
    let after = call(&mut client, "relay", "echo").await;
    assert_eq!(
        after["error"]["data"]["reason"], "program_not_running",
        "关掉以后连接断了：{after}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn only_extensions_register_methods_and_only_people_call_them() {
    let home = Home::new();
    let program = Program::new();
    let (record, keep) = crate::support::extensions::record(&home, "relay");
    let args: Vec<String> = [
        keep.as_str(),
        "hello",
        r#"ask:package.methods:{"methods":[{"name":"Bad"}]}"#,
        r#"ask:package.methods:{"methods":[{"name":"a"},{"name":"a"}]}"#,
        r#"ask:package.methods:{"methods":[{"name":"a","timeout_ms":10}]}"#,
        r#"ask:package.call:{"package":"relay","method":"echo"}"#,
        r#"ask:package.file:{"package":"relay","path":""}"#,
        "wait",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    install(&home, "relay", &program.name(), "manual", &args);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let mine = client
        .call(
            "m",
            "package.methods",
            json!({"methods": [{"name": "echo"}]}),
        )
        .await;
    assert_eq!(
        mine["error"]["data"]["reason"], "not_an_extension",
        "{mine}"
    );

    client
        .call("e", "package.enable", json!({"package": "relay"}))
        .await;
    let replies = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let text = std::fs::read_to_string(&record).unwrap_or_default();
            if text.matches("local_only").count() >= 2 {
                return text;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("扩展调了三样");
    assert_eq!(
        replies.matches("bad_params").count(),
        3,
        "名字写法不对、重复、时限不在范围里：{replies}"
    );
    core.stop_extensions().await;
}
