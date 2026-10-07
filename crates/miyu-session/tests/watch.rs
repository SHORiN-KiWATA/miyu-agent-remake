//! 「空了告诉我」，执行器这两头（施工 C-6，`docs/blueprint/cross-session.md` 第六条，2026-10-01 项目主人定改了
//! 「已经空着」那一条，`session/actor.md`「被等的名单」，`session/tools.md`「订、计时、再订」）。会话表是假的：
//! 记下每一次订、每一个命令。
//!
//! - 被等的这边：订进来时已经空着的不当场发，等它下一次忙完才发；起算时刻不晚于上一次忙完的时刻的（带话又订）
//!   照样当场发；正忙时订了，忙完就发；子代理没报完不发，报完、被叫醒的那一轮也做完了才发；同一个会话只记一个。
//! - 等的这边：效果落了盘才订（订的时刻就是那条结果的时刻）；那个会话不在了记 `gone`；载入再订；已经到点的载入时不订、当场
//!   作废；撤销以后不订、恢复撤销再订。

mod support;

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use miyu_kernel::event::{Body, ChildReason, ChildReported, Event, IdleReason};
use miyu_kernel::id::{AccountId, CommandId, JobId, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome};
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Handle, NotWatched, Pending, SessionPort};
use miyu_tool::MainSession;
use support::{Home, Lines, Opening, ask, say, stop, until_logged, within};

/// 被等的会话：短编号 `9f03b21c`。
const OTHER: &str = "0192f3a0-2222-7abc-8def-55669f03b21c";
/// 另一个在等的会话。
const WAITER: &str = "0192f3a0-3333-7abc-8def-0c5d77aa0c5d";
/// 派出去的子代理的子会话。
const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 一次订：被等的、在等的、起算时刻。
type Placed = (SessionId, SessionId, Timestamp);

/// 假的会话表：订的照 `gone` 回；命令都接受；记下每一次订、每一个命令。看得到的别的会话只有 `OTHER`。
#[derive(Default)]
struct Table {
    gone: bool,
    placed: Mutex<Vec<Placed>>,
    commands: Mutex<Vec<(SessionId, CommandId, By, Command)>>,
}

impl Table {
    fn placed(&self) -> Vec<Placed> {
        self.placed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 送出去的通知：发给谁、编号、谁发的、带的那一行。
    fn notices(&self) -> Vec<(SessionId, CommandId, By, Option<String>)> {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter_map(|(to, id, by, command)| match command {
                Command::PeerIdle { status } => {
                    Some((to.clone(), id.clone(), by.clone(), status.clone()))
                }
                _ => None,
            })
            .collect()
    }
}

impl SessionPort for Table {
    fn create(&self, _child: Child) -> Pending<'_, Result<SessionId, String>> {
        Box::pin(async { Ok(sid(CHILD)) })
    }

    fn open(&self, _session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn command(
        &self,
        session: SessionId,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, id, by, command));
        Box::pin(async { Ok(Outcome::Accepted { events: Vec::new() }) })
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
        _owner: AccountId,
        _stop: miyu_tool::Stop,
    ) -> Pending<'_, Result<Vec<MainSession>, String>> {
        let other = MainSession {
            id: sid(OTHER),
            title: String::new(),
            cwd: "~/src".to_string(),
            busy: true,
            last_active: Timestamp::parse("2026-10-01T09:00:00.000Z").expect("合写法"),
        };
        Box::pin(async move { Ok(vec![other]) })
    }

    fn read_log(&self, _session: SessionId) -> Pending<'_, Result<miyu_tool::Log, String>> {
        Box::pin(async { Err("no logs here".to_string()) })
    }

    fn held(&self, _session: SessionId) -> Pending<'_, bool> {
        Box::pin(async { false })
    }

    fn watch(
        &self,
        session: SessionId,
        watcher: SessionId,
        since: Timestamp,
    ) -> Pending<'_, Result<(), NotWatched>> {
        self.placed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, watcher, since));
        let gone = self.gone;
        Box::pin(async move {
            match gone {
                true => Err(NotWatched::Gone),
                false => Ok(()),
            }
        })
    }
}

fn sid(text: &str) -> SessionId {
    SessionId::parse(text).expect("合写法")
}

/// 真的基础系统：`send_message`、`subagent` 在里面。
fn basesystem(home: &Home) -> miyu_tool::Catalog {
    miyu_tool::Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出"))
        .expect("合写法")
}

/// 造一个主会话，会话表是 `table`。
async fn session(home: &Home, script: &Script, table: &Arc<Table>) -> Handle {
    let lines = Lines {
        sessions: Some(Arc::clone(table) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    home.create_full(script, &basesystem(home), Opening::default(), lines)
        .await
}

/// 载入会话 `id`，会话表是 `table`。
async fn reload(home: &Home, id: &SessionId, script: &Script, table: &Arc<Table>) -> Handle {
    let port = Arc::clone(table) as Arc<dyn SessionPort>;
    home.load_full(id, script, &basesystem(home), "~/src/miyu", Some(port))
        .await
}

/// 订 `OTHER`，只订不发。
fn watch_other() -> Play {
    let args = serde_json::json!({"to": OTHER, "notify_when_idle": true});
    Play::calls(&[("send_message", &args.to_string())])
}

/// 说一句，等第 `turns` 轮结束。
async fn turn(home: &Home, handle: &Handle, turns: usize) -> Vec<Event> {
    ask(handle, &format!("cmd-{turns}"), say("去办"))
        .await
        .expect("会话在跑");
    ended(home, handle.id(), turns).await
}

/// 等到磁盘上第 `turns` 轮结束。
async fn ended(home: &Home, id: &SessionId, turns: usize) -> Vec<Event> {
    until_logged(home, id, |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= turns
    })
    .await
}

/// 等到 `done` 成立，最多十秒。
async fn until(what: &str, done: impl Fn() -> bool) {
    within(what, async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}

/// 等一会儿，看不该来的没来。
async fn a_while() {
    tokio::time::sleep(Duration::from_millis(300)).await;
}

/// 订的那条结果的时刻。
fn watched_at(log: &[Event]) -> Timestamp {
    log.iter()
        .find(|event| matches!(&event.body, Body::ToolResult(result) if !result.effects.is_empty()))
        .expect("订了")
        .at
}

/// 日志里的 `peer.idle`，照先后：原因、谁记的。
fn notices(log: &[Event]) -> Vec<(IdleReason, By)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::PeerIdle(idle) => Some((idle.reason.clone(), event.by.clone())),
            _ => None,
        })
        .collect()
}

/// 这个会话发来的话的 `by`。
fn from(id: &SessionId) -> By {
    By::Session(Session { id: id.clone() })
}

#[tokio::test]
async fn the_watch_is_placed_once_its_effect_is_on_disk() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([watch_other(), Play::Says("等它。")]);
    let handle = session(&home, &script, &table).await;
    let log = turn(&home, &handle, 1).await;
    until("订", || !table.placed().is_empty()).await;
    assert_eq!(
        table.placed(),
        [(sid(OTHER), handle.id().clone(), watched_at(&log))],
        "订的是落了盘的那条结果的时刻"
    );
    assert!(
        table
            .commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty(),
        "只订不发：那边一个命令都没收到"
    );
    a_while().await;
    assert_eq!(table.placed().len(), 1, "一直在等的不再订");
}

#[tokio::test]
async fn a_session_that_is_gone_is_recorded_without_waking_her() {
    let home = Home::new();
    let table = Arc::new(Table {
        gone: true,
        ..Table::default()
    });
    let script = Script::new([watch_other(), Play::Says("等它。")]);
    let handle = session(&home, &script, &table).await;
    turn(&home, &handle, 1).await;
    let log = until_logged(&home, handle.id(), |log| !notices(log).is_empty()).await;
    assert_eq!(notices(&log), [(IdleReason::Gone, By::Kernel)]);
    a_while().await;
    let turns = home
        .log(handle.id())
        .iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .count();
    assert_eq!(turns, 1, "不在了只记下，不叫醒");
}

#[tokio::test]
async fn a_reload_places_the_watch_again_and_an_overdue_one_expires() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([watch_other(), Play::Says("等它。")]);
    let handle = session(&home, &script, &table).await;
    turn(&home, &handle, 1).await;
    until("订", || table.placed().len() == 1).await;
    let id = handle.id().clone();
    stop(&handle).await;
    drop(handle);
    let again = reload(&home, &id, &script, &table).await;
    until("载入以后再订", || table.placed().len() == 2).await;
    assert_eq!(table.placed()[0], table.placed()[1], "照日志再订，时刻不变");
    stop(&again).await;
    drop(again);
    // 日志整个往前挪 13 个小时：订的那一刻过了 12 小时。
    age(&home, &id, 13);
    let late = reload(&home, &id, &script, &table).await;
    let log = until_logged(&home, &id, |log| !notices(log).is_empty()).await;
    assert_eq!(
        notices(&log),
        [(IdleReason::Expired, By::Kernel)],
        "当场作废"
    );
    assert_eq!(table.placed().len(), 2, "到点的不订");
    drop(late);
}

#[tokio::test]
async fn undoing_the_watch_and_restoring_it() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([watch_other(), Play::Says("等它。")]);
    let handle = session(&home, &script, &table).await;
    turn(&home, &handle, 1).await;
    until("订", || table.placed().len() == 1).await;
    ask(&handle, "undo", Command::Revert { turn: None })
        .await
        .expect("在跑");
    a_while().await;
    assert_eq!(table.placed().len(), 1, "撤掉了不订");
    ask(&handle, "redo", Command::Unrevert).await.expect("在跑");
    until("恢复撤销以后再订", || table.placed().len() == 2).await;
    assert_eq!(table.placed()[0], table.placed()[1]);
}

// 原来这里断言「订进来已经空着就当场发」：2026-10-01 项目主人定改成下面两条——空着先不发（真模型撞见带出
// 的是旧回答），起算时刻不晚于忙完的时刻的（带话又订）照样当场发。

#[tokio::test]
async fn an_idle_session_waits_for_its_next_turn_to_send() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        Play::Says("  CI 修好了。\n细节在下面。"),
        Play::Says("又跑了一轮。"),
    ]);
    let handle = session(&home, &script, &table).await;
    turn(&home, &handle, 1).await;
    // 起算时刻在忙完以后才取，落进「已经空着」那一支。日志里有了 `turn.ended` 不等于已经空了：落盘以后还要更新派生的
    // 索引，才标成空闲。照它自己说的等它空下来，再过一会儿取起算时刻。
    while handle.busy() {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let since = support::now();
    handle.watch(sid(WAITER), since).expect("在跑");
    a_while().await;
    assert!(table.notices().is_empty(), "订进来的时候已经空着：不当场发");
    turn(&home, &handle, 2).await;
    until("通知", || table.notices().len() == 1).await;
    let this = handle.id().clone();
    let [(to, id, by, status)] = table.notices().try_into().unwrap();
    // 编号还是那次订的起算时刻，不是新忙完的时刻；带的是新那一轮的第一行，不是旧的。
    let expected_id = format!("{this}/idle/{WAITER}/{}", since.unix_millis());
    assert_eq!(to, sid(WAITER));
    assert_eq!(id.as_str(), expected_id);
    assert_eq!(by, from(&this));
    assert_eq!(status.as_deref(), Some("又跑了一轮。"));
    a_while().await;
    assert_eq!(table.notices().len(), 1, "发了就清掉");
}

#[tokio::test]
async fn watching_since_before_an_already_finished_turn_sends_at_once() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([Play::Says("CI 修好了。")]);
    let handle = session(&home, &script, &table).await;
    // 起算时刻在这一轮开始之前取，模拟「带话又订」这边手慢：那边先忙完才 `add_waiter`，照样当场发。
    let since = support::now();
    turn(&home, &handle, 1).await;
    handle.watch(sid(WAITER), since).expect("在跑");
    until("通知", || table.notices().len() == 1).await;
    let [(to, _, _, status)] = table.notices().try_into().unwrap();
    assert_eq!(to, sid(WAITER));
    assert_eq!(status.as_deref(), Some("CI 修好了。"));
}

// 正忙时订了，忙完就发：这一条没跟着 2026-10-01 的改动变。

#[tokio::test]
async fn a_busy_session_sends_it_when_its_turn_ends_and_keeps_one_per_watcher() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([Play::Holds]);
    let handle = session(&home, &script, &table).await;
    ask(&handle, "go", say("跑测试")).await.expect("在跑");
    let first = Timestamp::parse("2026-10-01T09:00:00.000Z").unwrap();
    let second = Timestamp::parse("2026-10-01T09:05:00.000Z").unwrap();
    handle.watch(sid(WAITER), first).expect("在跑");
    handle.watch(sid(WAITER), second).expect("在跑");
    handle.watch(sid(OTHER), first).expect("在跑");
    a_while().await;
    assert!(table.notices().is_empty(), "忙着不发");
    ask(
        &handle,
        "stop",
        Command::Interrupt {
            queued: miyu_kernel::session::Queued::Return,
        },
    )
    .await
    .expect("在跑");
    until("通知", || table.notices().len() == 2).await;
    a_while().await;
    let sent: Vec<(SessionId, String)> = table
        .notices()
        .into_iter()
        .map(|(to, id, ..)| (to, id.as_str().to_string()))
        .collect();
    let this = handle.id();
    assert_eq!(sent.len(), 2, "同一个会话只记一个：{sent:?}");
    assert!(sent.contains(&(
        sid(WAITER),
        format!("{this}/idle/{WAITER}/{}", second.unix_millis())
    )));
    assert!(sent.contains(&(
        sid(OTHER),
        format!("{this}/idle/{OTHER}/{}", first.unix_millis())
    )));
}

#[tokio::test]
async fn a_session_waiting_for_its_subagent_is_not_idle() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let args = serde_json::json!({"description": "查 CI", "prompt": "看看 CI 为什么红"});
    let script = Script::new([
        Play::calls(&[("subagent", &args.to_string())]),
        Play::Says("派出去了。"),
        Play::Says("子代理说修好了。"),
    ]);
    let handle = session(&home, &script, &table).await;
    turn(&home, &handle, 1).await;
    handle
        .watch(
            sid(WAITER),
            Timestamp::parse("2026-10-01T09:00:00.000Z").unwrap(),
        )
        .expect("在跑");
    a_while().await;
    assert!(table.notices().is_empty(), "子代理还没报，不算空");
    let reported = ChildReported {
        job: JobId::new(1).unwrap(),
        session: sid(CHILD),
        reason: ChildReason::Done,
        text: "CI 修好了。".to_string(),
        truncated: false,
        person: false,
        by_model: false,
    };
    let outcome = within(
        "回报",
        handle.command(
            CommandId::parse(&format!("{CHILD}/report/3")).unwrap(),
            from(&sid(CHILD)),
            Command::Report(reported),
        ),
    )
    .await
    .expect("在跑");
    assert!(matches!(outcome, Outcome::Accepted { .. }), "{outcome:?}");
    until("通知", || table.notices().len() == 1).await;
    let log = ended(&home, handle.id(), 2).await;
    assert!(
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            == 2,
        "回报叫醒的那一轮也做完了才发"
    );
    let [(_, _, _, status)] = table.notices().try_into().unwrap();
    assert_eq!(status.as_deref(), Some("子代理说修好了。"));
}

/// 把会话 `id` 的日志里每一条的时刻往前挪 `hours` 小时。
fn age(home: &Home, id: &SessionId, hours: i64) {
    let dir = home.root.session_dir(&support::alice_account(), id);
    for entry in std::fs::read_dir(&dir).expect("读得了会话目录") {
        let path = entry.expect("读得了").path();
        if path
            .extension()
            .is_none_or(|extension| extension != "jsonl")
        {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("读得了");
        let aged: Vec<String> = text
            .lines()
            .map(|line| {
                let mut event: serde_json::Value = serde_json::from_str(line).expect("一行一条");
                let at = Timestamp::parse(event["at"].as_str().expect("有时刻")).expect("合写法");
                let earlier = Timestamp::from_unix_millis(at.unix_millis() - hours * 3_600_000)
                    .expect("在范围里");
                event["at"] = serde_json::to_value(earlier).expect("写得成 JSON");
                serde_json::to_string(&event).expect("写得成 JSON")
            })
            .collect();
        std::fs::write(&path, aged.join("\n") + "\n").expect("写得了");
    }
}
