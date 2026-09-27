//! 几个测试共用的：临时的数据根、源码树里的资源目录、照剧本回的请求模型的端口、等一轮说完。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::accumulate::{Delta, Kind};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{
    Body, CallError, ErrorClass, Event, Level, Permission, TransientBody, Usage,
};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{AccountId, CommandId, ModelName, ProviderId, Seq, SessionId, VenueId};
use miyu_kernel::origin::{By, Model, Person};
use miyu_kernel::request::Request;
use miyu_kernel::session::{Command, Outcome};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_session::{
    Cancel, Create, Handle, Load, ModelPort, Pushed, Reports, Stopped, Subscription, create, load,
    new_id,
};
use miyu_store::env::{Env, Platform};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 一个用完就删的临时目录。
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-session-{}-{n}", std::process::id())))
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 一个临时的数据根，建好了骨架；源码树里的资源目录。
pub struct Home {
    pub scratch: Scratch,
    pub root: DataRoot,
    pub resources: ResourceRoot,
}

impl Home {
    pub fn new() -> Home {
        let scratch = Scratch::new();
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(scratch.0.clone().into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
        root.prepare().expect("临时目录里建得了骨架");
        Home {
            scratch,
            root,
            resources: ResourceRoot::at(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
            ),
        }
    }

    /// 造一个软件工程师的会话，请求模型照 `script` 回。造会话的命令编号是 `cmd-0`。
    pub async fn create(&self, script: Arc<Script>) -> Handle {
        let created = create(Create {
            root: &self.root,
            resources: &self.resources,
            id: new_id(now()),
            persona: "engineer",
            venue: VenueId::parse("local").expect("场所合写法"),
            owner: alice_account(),
            permission: Permission {
                level: Level::Workspace,
                read_only: false,
            },
            attended: true,
            environment: environment(),
            command: id("cmd-0"),
            by: alice(),
            model: script,
        });
        within("造会话", created).await.expect("造得出会话")
    }

    /// 载入会话 `session`，请求模型照 `script` 回。
    pub async fn load(&self, session: &SessionId, script: Arc<Script>) -> Handle {
        let loaded = load(Load {
            root: &self.root,
            owner: alice_account(),
            id: session.clone(),
            environment: environment(),
            model: script,
        });
        within("载入", loaded).await.expect("载入得了会话")
    }

    /// 磁盘上会话 `session` 的日志，照先后。
    pub fn log(&self, session: &SessionId) -> Vec<Event> {
        let dir = self.root.session_dir(&alice_account(), session);
        SessionLog::open(&dir, SEGMENT_LIMIT).expect("日志打得开").1
    }
}

/// 等 `what` 最多十秒：actor 出了毛病，测试几秒内就红，说清卡在哪，不一直等下去。
pub async fn within<T>(what: &str, future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(std::time::Duration::from_secs(10), future)
        .await
        .unwrap_or_else(|_| panic!("十秒内没等到{what}"))
}

/// 发命令 `command`，编号 `id`，等回应。
pub async fn ask(handle: &Handle, id_text: &str, command: Command) -> Result<Outcome, Stopped> {
    within(
        &format!("命令 {id_text} 的回应"),
        handle.command(id(id_text), alice(), command),
    )
    .await
}

/// 订阅。
pub async fn watch(handle: &Handle) -> Subscription {
    within("订阅", handle.subscribe())
        .await
        .expect("会话在跑，订阅得上")
}

/// 有计划地停下，等它停好。
pub async fn stop(handle: &Handle) {
    within("停下", handle.stop())
        .await
        .expect("会话在跑，停得下");
}

/// 一直读推送，读到第一段增量：请求在读流了。
pub async fn until_delta(subscription: &mut Subscription) {
    within("增量", async {
        loop {
            let next = subscription.next().await.expect("订阅没断");
            if matches!(&*next, Pushed::Transient(t) if matches!(t.body, TransientBody::ModelDelta(_)))
            {
                return;
            }
        }
    })
    .await;
}

/// 现在，照系统时间。
pub fn now() -> Timestamp {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("1970 年以后")
        .as_millis();
    Timestamp::from_unix_millis(i64::try_from(millis).expect("在范围里")).expect("在范围里")
}

pub fn alice_account() -> AccountId {
    AccountId::parse("alice").expect("账号合写法")
}

pub fn alice() -> By {
    By::Person(Person {
        account: alice_account(),
    })
}

pub fn environment() -> Environment {
    Environment {
        offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
        cwd: "~/src/miyu".to_string(),
    }
}

pub fn id(text: &str) -> CommandId {
    CommandId::parse(text).expect("命令编号合写法")
}

/// 说一句。
pub fn say(words: &str) -> Command {
    Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
    }
}

/// 等到磁盘上会话 `session` 的日志满足 `done`，最多五秒；交回那时的日志。
pub async fn until_logged(
    home: &Home,
    session: &SessionId,
    done: impl Fn(&[Event]) -> bool,
) -> Vec<Event> {
    let waited = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let log = home.log(session);
            if done(&log) {
                return log;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await;
    waited.unwrap_or_else(|_| panic!("十秒内磁盘上没等到：{:?}", kinds(&home.log(session))))
}

/// 事件的种类，照先后。
pub fn kinds(events: &[Event]) -> Vec<&str> {
    events.iter().map(|event| event.body.kind()).collect()
}

/// 一直读推送，读到回合结束那一条为止，交回读到的每一份。
pub async fn until_turn_ends(subscription: &mut Subscription) -> Vec<Arc<Pushed>> {
    let mut pushed = Vec::new();
    loop {
        let next = within("推送", subscription.next()).await.expect("订阅没断");
        let ended = matches!(
            &*next,
            Pushed::Events(events) if events.iter().any(|event| matches!(event.body, Body::TurnEnded(_)))
        );
        pushed.push(next);
        if ended {
            return pushed;
        }
    }
}

/// 剧本里的一次回复。
#[derive(Debug, Clone)]
pub enum Play {
    /// 说一句，说完。
    Says(&'static str),
    /// 出错：分类，供应商说要等多久。
    Fails {
        class: ErrorClass,
        wait_ms: Option<u64>,
    },
    /// 开了个头就停住，等叫停。
    Holds,
    /// 端口自己的 bug：一叫它就 panic。
    Panics,
}

/// 照剧本回的请求模型的端口。
pub struct Script {
    model: Model,
    plays: Mutex<VecDeque<Play>>,
    requests: Mutex<Vec<(Seq, Request)>>,
    cancelled: Arc<Mutex<Vec<Seq>>>,
}

impl Script {
    pub fn new(plays: impl IntoIterator<Item = Play>) -> Arc<Script> {
        Arc::new(Script {
            model: Model {
                endpoint: ProviderId::parse("deepseek").expect("端点合写法"),
                model: ModelName::parse("deepseek-v4").expect("模型名合写法"),
            },
            plays: Mutex::new(plays.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
            cancelled: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// 交给它的每一次请求，照先后：看到了第几条为止，和请求本身。
    pub fn requests(&self) -> Vec<(Seq, Request)> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 被叫停的请求，照先后。
    pub fn cancelled(&self) -> Vec<Seq> {
        self.cancelled
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl ModelPort for Script {
    fn model(&self) -> &Model {
        &self.model
    }

    fn call(&self, seen: Seq, request: Request, reports: Reports, cancel: Cancel) {
        let hash = request.hash();
        let asked = {
            let mut requests = self.requests.lock().unwrap_or_else(PoisonError::into_inner);
            requests.push((seen, request));
            requests.len()
        };
        let play = self
            .plays
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| panic!("剧本里没排第 {asked} 次请求说什么"));
        if matches!(play, Play::Panics) {
            panic!("端口自己的 bug");
        }
        let model = self.model.clone();
        let cancelled = Arc::clone(&self.cancelled);
        tokio::spawn(async move {
            reports.sent(model, hash);
            match play {
                Play::Says(text) => {
                    for delta in text_block(text, true) {
                        reports.delta(delta);
                    }
                    reports.ended(
                        Some(Usage {
                            uncached: 60,
                            cache_read: 40,
                            cache_write: 0,
                            output: 10,
                        }),
                        None,
                        None,
                    );
                }
                Play::Fails { class, wait_ms } => reports.ended(
                    None,
                    Some(CallError {
                        class,
                        message: "HTTP 429: slow down".to_string(),
                    }),
                    wait_ms,
                ),
                Play::Holds => {
                    for delta in text_block("…", false) {
                        reports.delta(delta);
                    }
                    cancel.wait().await;
                    cancelled
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(seen);
                }
                Play::Panics => unreachable!("上面已经 panic 了"),
            }
        });
    }
}

/// 一块正文：开始、全文；`ends` 的再收全。
fn text_block(text: &str, ends: bool) -> Vec<Delta> {
    let mut deltas = vec![
        Delta::Start {
            index: 0,
            kind: Kind::Text,
        },
        Delta::Text {
            index: 0,
            text: text.to_string(),
        },
    ];
    if ends {
        deltas.push(Delta::End { index: 0 });
    }
    deltas
}
