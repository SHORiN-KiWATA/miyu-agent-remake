//! 写不进去就停下（`07-存储.md` 第四节「写不进去」）：等着的命令收到「会话停了」，运行日志里记一条
//! `WARN`，写出错的种类；一轮在跑时停下的，不再算在跑（施工 3-9 上）。写盘的端口换成前几次写得进、
//! 之后磁盘满了的。
//!
//! 这个文件里只有这一个测试碰 actor：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，
//! 这里装的订阅者可能漏听。

use std::io;
use std::path::Path;
use std::time::Duration;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Event, Level, Permission};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{AccountId, ModelName, ProviderId, SessionId, VenueId};
use miyu_kernel::origin::{By, Model, Person};
use miyu_kernel::request::Request;
use miyu_kernel::session::Command;
use miyu_kernel::time::UtcOffset;
use miyu_log::{LevelFilter, Memory};
use miyu_policy::compose;
use miyu_store::resources::ResourceRoot;

use super::*;
use crate::handle::{Handle, Stopped};
use crate::port::{Cancel, Reports};

/// 前 `left` 次写得进，之后磁盘满了。
struct Failing {
    left: usize,
}

impl Store for Failing {
    fn append(&mut self, _: &[Event]) -> io::Result<()> {
        if self.left == 0 {
            return Err(io::ErrorKind::StorageFull.into());
        }
        self.left -= 1;
        Ok(())
    }

    fn events(&self) -> Result<Vec<Event>, String> {
        Err("读不回来".to_string())
    }

    fn events_from(&self, _: Seq) -> Result<Vec<Event>, String> {
        Err("读不回来".to_string())
    }
}

/// 请求模型的端口：一直不回，这一轮就一直在跑。
struct Holding(Model);

impl ModelPort for Holding {
    fn model(&self) -> Model {
        self.0.clone()
    }

    fn call(&self, _: Seq, _: Request, _: &crate::TurnConfig, _: Reports, _: Cancel) {}
}

/// 说一句 `text`。
fn say(text: &str) -> Command {
    Command::Send {
        blocks: vec![Block::Text(Text {
            text: text.to_string(),
        })],
        urgent: false,
        venue: None,
    }
}

fn alice() -> By {
    By::Person(Person::new(AccountId::parse("alice").expect("账号合写法")))
}

fn id(text: &str) -> CommandId {
    CommandId::parse(text).expect("命令编号合写法")
}

/// 等 `what` 最多十秒：出了毛病几秒内就红，不一直等下去。
async fn within<T>(what: &str, future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .unwrap_or_else(|_| panic!("十秒内没等到{what}"))
}

#[tokio::test]
async fn a_write_that_fails_stops_the_session() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let snapshot = compose(
        Some("engineer"),
        resources
            .sources_with(crate::testkit::sample_persona_texts())
            .expect("核心的字读得出来"),
        true,
    );
    let mut clock = Clock::default();
    let created = snapshot.session_created(
        AccountId::parse("alice").expect("账号合写法"),
        VenueId::parse("local").expect("场所合写法"),
        Permission {
            level: Level::Workspace,
            read_only: false,
        },
    );
    let environment = Environment {
        offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
        cwd: "~/src/miyu".to_string(),
        dirs: Vec::new(),
    };
    let (session, first) = Session::create(
        SessionId::parse("01a0f233-cfec-7023-8ed5-2a037a1d5ec8").expect("会话编号合写法"),
        id("cmd-0"),
        alice(),
        clock.now(),
        created,
        snapshot.policy().expect("出厂的快照造得出策略"),
        environment,
    );
    let model = Model {
        endpoint: ProviderId::parse("deepseek").expect("端点合写法"),
        model: ModelName::parse("deepseek-v4").expect("模型名合写法"),
    };
    let (inbox, mailbox) = mpsc::unbounded_channel();
    // 造会话那一条、第一句话写得进，第二句写不进。
    let run = snapshot.run_texts().expect("出厂的快照造得出两句");
    let lettering = Arc::new(crate::lettering::Lettering::new(
        run,
        snapshot.guard_texts().expect("出厂的快照造得出三句"),
    ));
    let guard = crate::guard::Guard::new(
        miyu_tool::Shelf::default(),
        std::path::PathBuf::new(),
        None,
        Arc::clone(&lettering),
        (false, crate::guard::Place::Venue, None),
    );
    let mut actor = Actor::new(
        session,
        Box::new(Failing { left: 2 }),
        Arc::new(Holding(model)),
        crate::tools::ToolKit {
            session: miyu_kernel::id::SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91")
                .expect("合写法"),
            owner: miyu_kernel::id::AccountId::parse("alice").expect("合写法"),
            confined: false,
            catalog: miyu_tool::Shelf::default(),
            lettering,
            home: None,
            data_root: std::path::PathBuf::new(),
            // 这个测试不跑工具：blob 不会存进去。
            blobs: miyu_store::blob::Blobs::new(std::path::PathBuf::new()),
            seen: miyu_tool::Seen::new(),
            sandbox: None,
            sandbox_cache: None,
            // 这个测试不跑工具：日志不会被读。
            log: miyu_tool::Log::new(crate::store::LogDir(std::path::PathBuf::new())),
            offset: miyu_kernel::time::UtcOffset::UTC,
            job_ids: Arc::new(crate::job_ids::JobIds::starting_after(None, 0)),
            agents: None,
            ledger: None,
            asks: false,
            memory: None,
        },
        crate::actor::JobKit {
            table: Arc::new(crate::jobs::Jobs::new()),
            // 这个测试不跑工具：不会起后台命令。
            dir: std::path::PathBuf::new(),
            blobs: miyu_store::blob::Blobs::new(std::path::PathBuf::new()),
            ids: Arc::new(crate::job_ids::JobIds::starting_after(None, 0)),
            roster: crate::jobs::Roster::default(),
            agents: None,
        },
        guard,
        mailbox,
        clock,
        crate::config::Turning::start(crate::fixed(Default::default()), String::new()).await,
    );
    let (reply, created) = oneshot::channel();
    actor.wait_for(id("cmd-0"), reply);
    let busy = actor.busy();
    let watched = actor.watched();
    let shown = actor.shown();
    let session = crate::new_id(Timestamp::from_unix_millis(0).expect("在范围里"));
    spawn(actor, first, span(&session));
    assert!(matches!(
        within("造会话的回应", created).await,
        Ok(Outcome::Accepted { .. })
    ));

    let ids = crate::handle::Ids {
        id: session.clone(),
        venue: miyu_kernel::id::VenueId::parse("local").expect("合写法"),
        owner: miyu_kernel::id::AccountId::parse("alice").expect("合写法"),
    };
    let handle = Handle::new(ids, inbox, busy, false, watched, shown);
    assert!(!handle.busy(), "刚造出来，没有回合");
    let first = within(
        "第一句的回应",
        handle.command(id("cmd-1"), alice(), say("你好")),
    )
    .await;
    assert!(matches!(first, Ok(Outcome::Accepted { .. })), "{first:?}");
    within("这一轮在跑", async {
        while !handle.busy() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let said = within(
        "第二句的回应",
        handle.command(id("cmd-2"), alice(), say("在吗")),
    )
    .await;
    assert_eq!(said, Err(Stopped), "等着的命令收到「会话停了」");
    within("停了以后不算在跑", async {
        while handle.busy() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(
        within("订阅", handle.subscribe()).await.is_err(),
        "停了的会话订阅不了"
    );

    let lines = memory.lines();
    let warned = format!(
        " WARN  session  {} write failed, stopped kind=StorageFull",
        session.as_str()
    );
    assert!(
        lines.iter().any(|line| line.ends_with(&warned)),
        "{lines:#?}"
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("你好") || line.contains("在吗")),
        "日志里没有对话的字：{lines:#?}"
    );
}
