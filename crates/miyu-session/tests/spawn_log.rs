//! 派子代理的运行日志（施工 7-5，`docs/blueprint/session/actor.md`「运行日志」）：派出去了记一行，造不成、交代送不进去各记
//! 一行 `WARN` 带原因；标题、交代的字一个都不记。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听。

mod support;

use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome, Reason};
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Pending, SessionPort};
use miyu_tool::Catalog;
use support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};

/// 假的会话表：第 1 次造不成，第 2 次造成了、交代被拒，第 3 次都成。
#[derive(Default)]
struct Table(Mutex<usize>);

impl SessionPort for Table {
    fn create(&self, _child: Child) -> Pending<'_, Result<SessionId, String>> {
        let mut made = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        *made += 1;
        let answer = match *made {
            1 => Err("disk full".to_string()),
            n => Ok(
                SessionId::parse(&format!("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d9{n}"))
                    .expect("合写法"),
            ),
        };
        Box::pin(async move { answer })
    }

    fn command(
        &self,
        _session: SessionId,
        _id: CommandId,
        _by: By,
        _command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        let made = *self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let answer = match made {
            2 => Outcome::Rejected {
                reason: Reason::EmptyMessage,
            },
            _ => Outcome::Accepted { events: Vec::new() },
        };
        Box::pin(async move { Ok(answer) })
    }
}

#[tokio::test]
async fn the_log_says_what_was_started_and_why_not() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(home.resources.path()).unwrap()).unwrap();
    let call = |n: u32| {
        let args =
            serde_json::json!({"description": format!("紫色的标题{n}"), "prompt": "紫色的交代"});
        Play::calls(&[("agent", &args.to_string())])
    };
    let script = Script::new([call(1), call(2), call(3), Play::Says("好。")]);
    let lines = Lines {
        sessions: Some(Arc::new(Table::default()) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &tools, Opening::default(), lines)
        .await;
    let session = handle.id().clone();
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.unwrap();
    until_turn_ends(&mut pushes).await;

    let lines = memory.lines();
    let spawned: Vec<String> = lines
        .iter()
        .filter(|line| line.contains(session.as_str()) && line.contains("subagent"))
        .map(|line| {
            // 去掉日子、时刻、会话编号：每次不一样。
            let words: Vec<&str> = line.split_whitespace().collect();
            [&words[2..4], &words[5..]].concat().join(" ")
        })
        .collect();
    assert_eq!(
        spawned,
        [
            "WARN session subagent not created job=j1 error=\"disk full\"",
            "WARN session subagent not given its task job=j2 child=01a0d78c-ca52-7d19-8b64-0e3f5a7c2d92 error=\"the task was refused: empty_message\"",
            "INFO session subagent started job=j3 child=01a0d78c-ca52-7d19-8b64-0e3f5a7c2d93",
        ],
        "{lines:#?}"
    );
    for line in &lines {
        assert!(!line.contains("紫色"), "标题、交代的字不进日志：{line}");
    }
}
