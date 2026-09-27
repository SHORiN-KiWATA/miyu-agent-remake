//! 几个测试共用的：临时的数据根、源码树里的资源目录、带时限的等待、等一轮说完。剧本端口在
//! `miyu_session::testkit`。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event, Level, Permission, TransientBody};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::{By, Person};
use miyu_kernel::session::{Command, Outcome};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_session::{
    Create, Handle, Load, Models, Pushed, Stopped, Subscription, create, load, new_id,
};
use miyu_store::env::{Env, Platform};
use miyu_store::log::read_events;
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

    /// 造一个软件工程师的会话，请求模型的端口由 `models` 造。造会话的命令编号是 `cmd-0`。
    pub async fn create(&self, models: &dyn Models) -> Handle {
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
            oneshot: false,
            environment: environment(),
            command: id("cmd-0"),
            by: alice(),
            models,
        });
        within("造会话", created).await.expect("造得出会话")
    }

    /// 载入会话 `session`，请求模型的端口由 `models` 造。
    pub async fn load(&self, session: &SessionId, models: &dyn Models) -> Handle {
        let loaded = load(Load {
            root: &self.root,
            owner: alice_account(),
            id: session.clone(),
            environment: environment(),
            models,
        });
        within("载入", loaded).await.expect("载入得了会话")
    }

    /// 磁盘上会话 `session` 的日志，照先后。
    pub fn log(&self, session: &SessionId) -> Vec<Event> {
        let dir = self.root.session_dir(&alice_account(), session);
        read_events(&dir).expect("日志读得出")
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
