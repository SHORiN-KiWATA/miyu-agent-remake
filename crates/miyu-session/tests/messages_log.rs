//! 留言的运行日志（施工 7-7，`docs/blueprint/session/actor.md`「运行日志」）：送到了记一行 `INFO`，送不到记一行 `WARN` 带
//! 原因；发给谁写 `parent` 或者任务编号；留言的字一个都不记。她认错了的（没有父、不是她派的）不记：那是她的事，结果里说了。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听。

mod support;

use std::sync::Arc;

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome, Reason};
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Lineage, Pending, SessionPort};
use miyu_tool::Catalog;
use support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};

/// 假的会话表：派的子会话收留言，父会话拒收。
struct Table;

/// 父会话。
const PARENT: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d99";

impl SessionPort for Table {
    fn create(&self, _child: Child) -> Pending<'_, Result<SessionId, String>> {
        let child = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91").expect("合写法");
        Box::pin(async move { Ok(child) })
    }

    fn open(&self, _session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    /// 这几份假的会话表用不到读别的会话的日志（施工 C-4）。
    fn read_log(&self, _session: SessionId) -> Pending<'_, Result<miyu_tool::Log, String>> {
        Box::pin(async { Err("no logs here".to_string()) })
    }

    /// 这份假的会话表没有一次性会话（施工 C-5）：用不到的时候一律照送到了算。
    fn held(&self, _session: SessionId) -> Pending<'_, bool> {
        Box::pin(async { false })
    }

    /// 这个测试不订「空了告诉我」（施工 C-6）。
    fn watch(
        &self,
        _session: SessionId,
        _watcher: SessionId,
        _since: miyu_kernel::time::Timestamp,
    ) -> Pending<'_, Result<(), miyu_session::NotWatched>> {
        Box::pin(async {
            Err(miyu_session::NotWatched::Failed(
                "no watches here".to_string(),
            ))
        })
    }

    fn command(
        &self,
        session: SessionId,
        _id: CommandId,
        _by: By,
        _command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        let answer = match session.as_str() == PARENT {
            true => Outcome::Rejected {
                reason: Reason::Restoring,
                about: Vec::new(),
            },
            false => Outcome::Accepted { events: Vec::new() },
        };
        Box::pin(async move { Ok(answer) })
    }
    fn stop(
        &self,
        _session: SessionId,
        _id: CommandId,
        _by: By,
    ) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn peek(&self, _session: SessionId) -> Pending<'_, Result<miyu_session::Peek, String>> {
        Box::pin(async { Ok(miyu_session::Peek::default()) })
    }

    fn sessions(
        &self,
        _owner: miyu_kernel::id::AccountId,
        _stop: miyu_tool::Stop,
    ) -> Pending<'_, Result<Vec<miyu_tool::MainSession>, String>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[tokio::test]
async fn the_log_says_where_it_went_and_why_not() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(home.resources.path()).unwrap()).unwrap();
    let call = |name: &str, args: serde_json::Value| Play::calls(&[(name, &args.to_string())]);
    let script = Script::new([
        call(
            "subagent",
            serde_json::json!({"description": "甲", "prompt": "Do it."}),
        ),
        call(
            "send_message",
            serde_json::json!({"to": "j1", "message": "紫色的留言"}),
        ),
        call(
            "send_message",
            serde_json::json!({"to": "parent", "message": "紫色的留言"}),
        ),
        call(
            "send_message",
            serde_json::json!({"to": "j9", "message": "紫色的留言"}),
        ),
        Play::Says("好。"),
    ]);
    let lines = Lines {
        sessions: Some(Arc::new(Table) as Arc<dyn SessionPort>),
        lineage: Some(Lineage {
            parent: SessionId::parse(PARENT).unwrap(),
            depth: 1,
        }),
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
    let sent: Vec<String> = lines
        .iter()
        .filter(|line| line.contains(session.as_str()) && line.contains(" message "))
        .map(|line| {
            // 去掉日子、时刻、会话编号：每次不一样。
            let words: Vec<&str> = line.split_whitespace().collect();
            [&words[2..4], &words[5..]].concat().join(" ")
        })
        .collect();
    assert_eq!(
        sent,
        [
            "INFO session message sent to=j1",
            "WARN session message not delivered to=parent error=\"refused: restoring\"",
        ],
        "{lines:#?}"
    );
    for line in &lines {
        assert!(!line.contains("紫色"), "留言的字不进日志：{line}");
    }
}
