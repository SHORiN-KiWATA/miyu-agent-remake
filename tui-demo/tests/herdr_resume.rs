//! 真 herdr 恢复验收：临时 MIYU_HOME、隔离 XDG 配置、假模型；不接触用户的默认服务。

#![cfg(target_os = "linux")]
mod support;
use miyu_session::testkit::{Play, Script};
use serde_json::Value;
use std::{
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
use support::{Home, Tui};

struct Herdr<'a> {
    home: &'a Home,
    vars: Vec<(String, String)>,
}
impl Herdr<'_> {
    fn env(&self) -> Vec<(&str, &str)> {
        self.vars
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect()
    }
    fn api(&self, args: &[&str]) -> Value {
        let out = Command::new("herdr")
            .args(["--session", "miyu-resume-test"])
            .args(args)
            .envs(self.env())
            .output()
            .expect("隔离 herdr 验收前提成立");
        assert!(
            out.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(if out.stdout.is_empty() {
            b"null"
        } else {
            &out.stdout
        })
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&out.stdout).into_owned()))
    }
    fn client(&self) -> Tui {
        Tui::program(
            self.home.root(),
            &self.home.work,
            "zh_CN.UTF-8",
            &self.env(),
            &["--session", "miyu-resume-test"],
            "herdr",
        )
    }
    fn wait(&self, pane: &str, text: &str) {
        let until = Instant::now() + Duration::from_secs(20);
        loop {
            let v = self.api(&["pane", "read", pane]);
            if v.to_string().contains(text) {
                return;
            }
            assert!(Instant::now() < until, "{pane}: {v}");
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
impl Drop for Herdr<'_> {
    fn drop(&mut self) {
        let _stopped = Command::new("herdr")
            .args(["--session", "miyu-resume-test", "server", "stop"])
            .envs(self.env())
            .output();
    }
}

#[test]
#[ignore = "需要已安装 herdr；仅 Linux 的隔离恢复实测"]
fn real_herdr_restores_two_panes_to_their_own_sessions() {
    let home = Home::new(Script::new([
        Play::Says("pane_one_answer"),
        Play::Says("pane_two_answer"),
    ]));
    let bin = home.root().join("bin");
    std::fs::create_dir_all(&bin).expect("隔离 herdr 验收前提成立");
    std::os::unix::fs::symlink(
        env!("CARGO_BIN_EXE_miyu-tui-demo"),
        bin.join("miyu-tui-demo"),
    )
    .expect("隔离 herdr 验收前提成立");
    let config = home.root().join("herdr-config/herdr");
    std::fs::create_dir_all(&config).expect("隔离 herdr 验收前提成立");
    std::fs::write(
        config.join("config.toml"),
        "[general]\ndefault_shell = \"/bin/sh\"\nshell_mode = \"non_login\"\n",
    )
    .expect("隔离 herdr 验收前提成立");
    let h = Herdr {
        home: &home,
        vars: vec![
            (
                "XDG_CONFIG_HOME".into(),
                home.root().join("herdr-config").display().to_string(),
            ),
            (
                "XDG_STATE_HOME".into(),
                home.root().join("herdr-state").display().to_string(),
            ),
            (
                "XDG_CACHE_HOME".into(),
                home.root().join("cache").display().to_string(),
            ),
            ("MIYU_HOME".into(), home.root().display().to_string()),
            (
                "MIYU_RESOURCES".into(),
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../resources")
                    .display()
                    .to_string(),
            ),
            (
                "PATH".into(),
                format!(
                    "{}:{}",
                    bin.display(),
                    std::env::var("PATH").expect("隔离 herdr 验收前提成立")
                ),
            ),
            ("SHELL".into(), "/bin/sh".into()),
            ("HERDR_DISABLE_SOUND".into(), "1".into()),
        ],
    };
    let mut client = h.client();
    client.pump(Duration::from_secs(2));
    let panes = h.api(&["pane", "list"]);
    let p1 = panes["result"]["panes"][0]["pane_id"]
        .as_str()
        .expect("隔离 herdr 验收前提成立");
    let split = h.api(&["pane", "split", p1, "--direction", "right"]);
    let p2 = split["result"]["pane"]["pane_id"]
        .as_str()
        .expect("隔离 herdr 验收前提成立");
    for (pane, question, answer) in [
        (p1, "pane_one_question", "pane_one_answer"),
        (p2, "pane_two_question", "pane_two_answer"),
    ] {
        h.api(&["pane", "run", pane, "miyu-tui-demo"]);
        h.wait(pane, "Tab");
        h.api(&["pane", "run", pane, question]);
        h.wait(pane, answer);
    }
    std::thread::sleep(Duration::from_millis(300));
    let first = h.api(&["pane", "get", p1]);
    let second = h.api(&["pane", "get", p2]);
    assert_eq!(first["result"]["pane"]["agent"], "miyu");
    assert_eq!(second["result"]["pane"]["agent"], "miyu");
    h.api(&["server", "stop"]);
    drop(client);
    let mut restored = h.client();
    restored.pump(Duration::from_secs(3));
    h.wait(p1, "pane_one_answer");
    h.wait(p2, "pane_two_answer");
    assert!(
        !h.api(&["pane", "read", p1])
            .to_string()
            .contains("pane_two_answer")
    );
    assert!(
        !h.api(&["pane", "read", p2])
            .to_string()
            .contains("pane_one_answer")
    );
    for (pane, before) in [(p1, &first), (p2, &second)] {
        let after = h.api(&["pane", "get", pane]);
        assert_eq!(after["result"]["pane"]["agent"], "miyu");
        assert_ne!(
            before["result"]["pane"]["terminal_id"],
            after["result"]["pane"]["terminal_id"]
        );
    }
    // /new 后不曾开口：重启只能回到 shell，不能复活旧会话。
    h.api(&["pane", "run", p1, "/new"]);
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        !h.api(&["pane", "read", p1])
            .to_string()
            .contains("pane_one_answer")
    );
    h.api(&["server", "stop"]);
    drop(restored);
    let mut cleared = h.client();
    cleared.pump(Duration::from_secs(3));
    h.wait(p1, "$");
    assert!(h.api(&["pane", "get", p1])["result"]["pane"]["agent"].is_null());
    assert!(
        !h.api(&["pane", "read", p1])
            .to_string()
            .contains("pane_one_answer")
    );
    h.wait(p2, "pane_two_answer");
}
