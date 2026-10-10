//! 跑包自己的检查（施工 9-2，`docs/blueprint/packages.md`）：真核心走一遍。`check` 不写文件时，照起来时读到的清单跑有
//! `[check]` 的包，收它一行一个的问题，接在核心自己查的后面；格式不对的行、跑坏了的、程序没找到的各报一条警告。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 管理员（测试里是 alice）家目录里的一份清单：子命令 `name` 跑 `program`，检查带 `args`。
fn install(home: &Home, id: &str, program: &str, args: &[&str]) {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        &format!("home/alice/packages/{id}/package.toml"),
        &format!(
            "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = {{ en = \"P\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"P\" }}\n\n[process]\n\n[check]\nargs = [{}]\n",
            args.join(", ")
        ),
    );
}

async fn checked(home: &Home) -> Vec<Value> {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client.call("c1", "check", json!({})).await;
    reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .clone()
}

#[tokio::test]
async fn a_missing_program_is_one_warning() {
    let home = Home::new();
    install(&home, "ghost", "miyu-no-such-program-anywhere", &["check"]);
    let problems = checked(&home).await;
    let ghost: Vec<&Value> = problems
        .iter()
        .filter(|problem| problem["file"] == "home/alice/packages/ghost/package.toml")
        .collect();
    assert_eq!(
        ghost,
        [&json!({
            "kind": "package",
            "file": "home/alice/packages/ghost/package.toml",
            "code": "check_unavailable",
            "level": "warning",
            "message": "没找到 miyu-no-such-program-anywhere，这个包自己的检查没跑",
        })]
    );
}

/// 测试程序旁边的一个临时脚本（Unix）：包的程序只找主程序旁边的，测试里的主程序就是测试程序自己。用完删掉。
#[cfg(unix)]
struct Fixture(std::path::PathBuf);

#[cfg(unix)]
impl Fixture {
    fn new(name: &str, body: &str) -> Fixture {
        use std::os::unix::fs::PermissionsExt;
        let exe = std::env::current_exe().expect("找得到测试程序");
        let dir = std::fs::canonicalize(&exe)
            .expect("测试程序在")
            .parent()
            .expect("有上一级")
            .to_path_buf();
        let path = dir.join(format!("{name}-{}", std::process::id()));
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("写得进");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("改得了");
        Fixture(path)
    }

    fn name(&self) -> String {
        self.0
            .file_name()
            .expect("有名字")
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(unix)]
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(std::fs::remove_file(&self.0));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_package_check_reports_its_problems_after_the_cores_own() {
    let home = Home::new();
    let line = r#"{"kind":"venue","file":"system/venues.d/x.toml","line":2,"level":"error","message":"bad rule"}"#;
    let bridge = Fixture::new(
        "miyu-test-check-bridge",
        &format!("echo '{line}'\necho garbage\nexit 1"),
    );
    let broken = Fixture::new("miyu-test-check-broken", "exit 7");
    install(&home, "bridge", &bridge.name(), &["check"]);
    install(&home, "broken", &broken.name(), &["check"]);
    let problems = checked(&home).await;
    let from_packages: Vec<&Value> = problems
        .iter()
        .filter(|problem| {
            problem["kind"] == "venue"
                || problem["code"]
                    .as_str()
                    .is_some_and(|code| code.starts_with("check_"))
        })
        .collect();
    assert_eq!(
        from_packages,
        [
            &json!({"kind":"venue","file":"system/venues.d/x.toml","line":2,"level":"error","message":"bad rule"}),
            &json!({"kind":"package","file":"home/alice/packages/bridge/package.toml","code":"check_output","level":"warning","message":"这个包自己的检查印了 1 行看不懂的，跳过了"}),
            &json!({"kind":"package","file":"home/alice/packages/broken/package.toml","code":"check_failed","level":"warning","message":"这个包自己的检查没跑完：exit code 7"}),
        ],
        "照编号的先后，接在核心自己查的后面"
    );
}
