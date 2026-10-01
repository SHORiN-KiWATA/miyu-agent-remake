//! 回合的配置（施工 8-4，`docs/blueprint/config.md`「怎么走」第八条第 3 条，G7）：回合开始时冻结一份，这一轮的每一次请求
//! （出错再来的也算）都照它；回合中途改了配置，下一轮才用上。每一轮开始都照会话这时的目录重新取一次（项目配置每一轮
//! 重读）。
//!
//! 配置的来源是假的：`log.level` 取自一份「项目配置」的字，每次取都重新读那一份文件，记下照哪个目录取的。

mod support;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use miyu_config::merge::{Layers, Resolved, merge};
use miyu_config::parse::parse;
use miyu_config::secret::{Reference, Secret};
use miyu_config::{Item, Layer};
use miyu_kernel::event::ErrorClass;
use miyu_kernel::id::Seq;
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::Limits;
use miyu_log::settings::LogSettings;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Cancel, ConfigSource, ForSession, ModelPort, Models, Reports, TurnConfig};
use support::{Home, Scratch, ask, environment, say, until_turn_ends, watch as watching};

/// 一份最终值：`log.level` 是 `level`。
fn resolved(level: &str) -> Resolved {
    let items: &[Item] = LogSettings::ITEMS;
    let text = format!("[log]\nlevel = \"{level}\"\n");
    let parsed = parse(items, Layer::System, &text).expect("读得懂");
    let layers = Layers {
        system: Some(&parsed),
        personal: None,
        project: None,
    };
    merge(items, &layers, &|_| None)
}

/// 一份配置里 `log.level` 的最终值。
fn level(config: &TurnConfig) -> String {
    LogSettings::from(&config.resolved.values()).level
}

/// 假的来源：每次取都重新读 `file`（里面写着级别），记下照哪个目录取的。
#[derive(Debug)]
struct Source {
    file: PathBuf,
    asked: Arc<Mutex<Vec<String>>>,
}

impl ConfigSource for Source {
    fn with_project(&self, dir: &str) -> Resolved {
        self.asked.lock().expect("拿得到锁").push(dir.to_string());
        resolved(std::fs::read_to_string(&self.file).expect("读得到").trim())
    }

    fn secret(&self, _: &Reference) -> Option<Secret> {
        None
    }
}

/// 不变的来源。
#[derive(Debug)]
struct Fixed(&'static str);

impl ConfigSource for Fixed {
    fn with_project(&self, _: &str) -> Resolved {
        resolved(self.0)
    }

    fn secret(&self, _: &Reference) -> Option<Secret> {
        None
    }
}

/// 换配置的那一头，和换成的那一份。
type Switch = (watch::Sender<Arc<dyn ConfigSource>>, Arc<dyn ConfigSource>);

/// 第一次请求交到端口时，把配置换成 `then`：回合正在进行时改了配置。
struct Switching {
    script: Script,
    switch: Mutex<Option<Switch>>,
}

impl Models for Switching {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(Switching {
            script: self.script.clone(),
            switch: Mutex::new(self.switch.lock().expect("拿得到锁").take()),
        })
    }
}

impl ModelPort for Switching {
    fn model(&self) -> Model {
        self.script.model()
    }

    fn limits(&self) -> Limits {
        self.script.limits()
    }

    fn call(
        &self,
        seen: Seq,
        request: Request,
        config: &TurnConfig,
        reports: Reports,
        cancel: Cancel,
    ) {
        if let Some((sender, then)) = self.switch.lock().expect("拿得到锁").take() {
            sender.send_replace(then);
        }
        self.script.call(seen, request, config, reports, cancel);
    }
}

#[tokio::test]
async fn a_change_mid_turn_waits_for_the_next_turn() {
    let mut home = Home::new();
    let first: Arc<dyn ConfigSource> = Arc::new(Fixed("warn"));
    let (sender, configs) = watch::channel(first);
    home.configs = configs;
    let script = Script::new([
        // 出错再来：同一轮里的第二次请求。
        Play::Fails {
            class: ErrorClass::RateLimited,
            wait_ms: Some(20),
        },
        Play::Says("好了。"),
        Play::Says("又好了。"),
    ]);
    let models = Switching {
        script: script.clone(),
        switch: Mutex::new(Some((sender, Arc::new(Fixed("error"))))),
    };
    let handle = home.create(&models).await;
    let mut pushes = watching(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    ask(&handle, "cmd-2", say("again")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let levels: Vec<String> = script.configs().iter().map(level).collect();
    assert_eq!(
        levels,
        ["warn", "warn", "error"],
        "第一轮的两次都照开始时的，中途改的下一轮才用上"
    );
}

#[tokio::test]
async fn every_turn_reads_the_project_config_again() {
    let scratch = Scratch::new();
    std::fs::create_dir_all(&scratch.0).unwrap();
    let file = scratch.0.join("level.txt");
    std::fs::write(&file, "warn").unwrap();
    let asked = Arc::new(Mutex::new(Vec::new()));
    let source: Arc<dyn ConfigSource> = Arc::new(Source {
        file: file.clone(),
        asked: Arc::clone(&asked),
    });
    let mut home = Home::new();
    // 发送的一头丢了也照样取得到最后那一份：这里不换，只改「项目配置」那份文件。
    home.configs = watch::channel(source).1;
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let handle = home.create(&script).await;
    let mut pushes = watching(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    std::fs::write(&file, "debug").unwrap();
    ask(&handle, "cmd-2", say("again")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let levels: Vec<String> = script.configs().iter().map(level).collect();
    assert_eq!(levels, ["warn", "debug"], "下一轮重新读了");
    let cwd = environment().cwd;
    assert_eq!(
        *asked.lock().unwrap(),
        [cwd.clone(), cwd.clone(), cwd],
        "造会话时一次，每一轮开始一次，都照会话这时的目录"
    );
}
