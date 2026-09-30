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
    Create, Handle, Jobs, Lineage, Load, Models, Pushed, SandboxCache, SessionPort, Stopped,
    Subscription, create, load, new_id,
};
use miyu_store::env::{Env, Platform};
use miyu_store::log::read_events;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

/// 一个用完就删的临时目录。
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new() -> Scratch {
        Scratch::under(&std::env::temp_dir())
    }

    /// 放在 cargo 给集成测试的 `target/tmp` 下面，不在系统的临时目录里（施工 4-3 下）：临时目录整个能读能写，
    /// 放在里面就造不出「边界以外」。
    pub fn outside_temp() -> Scratch {
        Scratch::under(Path::new(env!("CARGO_TARGET_TMPDIR")))
    }

    fn under(dir: &Path) -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(dir.join(format!("miyu-session-{}-{n}", std::process::id())))
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

/// 一个临时的数据根，建好了骨架；源码树里的资源目录；一个假的系统家目录（施工 4-3 下）。
pub struct Home {
    pub scratch: Scratch,
    pub root: DataRoot,
    pub resources: ResourceRoot,
    pub home: PathBuf,
    /// 执行器的任务表（施工 7-3）：这个场地里的会话共用一张，和核心里一样。
    pub jobs: Arc<Jobs>,
}

/// 造会话时可以换的几样（施工 4-3 下）。
pub struct Opening {
    /// 开始时的权限。
    pub permission: Permission,
    /// 有没有人能确认。
    pub attended: bool,
    /// 工作目录。
    pub cwd: String,
    /// 加进来的目录（施工 5-10 上）：和工作区一样能读能写。
    pub dirs: Vec<String>,
    /// 沙盒的助手：有的当这台机器上的沙盒能用（施工 5-4 上）。假工具不起它，随便一条路径就行；真的起命令的用
    /// [`miyu_sandbox::testkit::built_helper`]。
    pub sandbox: Option<PathBuf>,
    /// 沙盒的缓存（施工 5-4 下）：没有的沙盒里不设工具链的变量。
    pub sandbox_cache: Option<SandboxCache>,
}

/// 造会话时另外可以换的几样：派子代理用的三样（施工 7-5），一次性的（施工 7-9）。
pub struct Lines {
    /// 场所：默认在本机。
    pub venue: VenueId,
    /// 父会话和第几层：子会话才有。
    pub lineage: Option<Lineage>,
    /// 造子会话、给别的会话发命令的端口：没有的派不了子代理。
    pub sessions: Option<Arc<dyn SessionPort>>,
    /// 造会话的命令编号：默认 `cmd-0`；子会话向上回报时照它读回任务编号（`<父会话>/<编号>`，施工 7-6）。
    pub command: Option<CommandId>,
    /// 一次性的（`miyu ask` 开的那种）：没有头订阅着时回报只记下（施工 7-9）。默认不是。
    pub oneshot: bool,
}

impl Default for Lines {
    /// 本机的主会话，派不了子代理。
    fn default() -> Lines {
        Lines {
            venue: VenueId::parse("local").expect("场所合写法"),
            lineage: None,
            sessions: None,
            command: None,
            oneshot: false,
        }
    }
}

impl Default for Opening {
    /// 工作区这一级，有人能确认，工作目录照 [`environment`]，沙盒用不了。
    fn default() -> Opening {
        Opening {
            permission: Permission {
                level: Level::Workspace,
                read_only: false,
            },
            attended: true,
            cwd: environment().cwd,
            dirs: Vec::new(),
            sandbox: None,
            sandbox_cache: None,
        }
    }
}

impl Home {
    pub fn new() -> Home {
        Home::in_scratch(Scratch::new())
    }

    /// 场地不在系统的临时目录里（施工 4-3 下）：数据根是 `data/`，假的家是 `home/`，另有工作区 `work/`、
    /// 边界以外的 `other/`，都在 [`Scratch::outside_temp`] 里。
    pub fn outside_temp() -> Home {
        let home = Home::in_scratch(Scratch::outside_temp());
        for dir in ["work", "other"] {
            std::fs::create_dir_all(home.scratch.0.join(dir)).expect("建得了目录");
        }
        home
    }

    fn in_scratch(scratch: Scratch) -> Home {
        std::fs::create_dir_all(&scratch.0).expect("建得了临时目录");
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(scratch.0.join("data").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
        root.prepare().expect("临时目录里建得了骨架");
        let home = scratch.0.join("home");
        std::fs::create_dir_all(&home).expect("建得了假的家");
        Home {
            scratch,
            root,
            home,
            jobs: Arc::new(Jobs::new()),
            resources: ResourceRoot::at(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
            ),
        }
    }

    /// 造一个软件工程师的会话，请求模型的端口由 `models` 造，没有工具。造会话的命令编号是 `cmd-0`。
    pub async fn create(&self, models: &dyn Models) -> Handle {
        self.create_with(models, &Catalog::default()).await
    }

    /// 造一个软件工程师的会话，工具面照目录 `tools`（施工 4-1）。
    pub async fn create_with(&self, models: &dyn Models, tools: &Catalog) -> Handle {
        self.create_as(models, tools, Opening::default()).await
    }

    /// 造一个软件工程师的会话，权限、有没有人能确认、工作目录照 `opening`（施工 4-3 下）。
    pub async fn create_as(
        &self,
        models: &dyn Models,
        tools: &Catalog,
        opening: Opening,
    ) -> Handle {
        self.create_full(models, tools, opening, Lines::default())
            .await
    }

    /// 同 [`Home::create_as`]，场所、父会话、造子会话的端口照 `lines`（施工 7-5）。
    pub async fn create_full(
        &self,
        models: &dyn Models,
        tools: &Catalog,
        opening: Opening,
        lines: Lines,
    ) -> Handle {
        let created = create(Create {
            root: &self.root,
            resources: &self.resources,
            id: new_id(now()),
            persona: "engineer",
            venue: lines.venue,
            owner: alice_account(),
            permission: opening.permission,
            attended: opening.attended,
            oneshot: lines.oneshot,
            environment: Environment {
                cwd: opening.cwd,
                dirs: opening.dirs,
                ..environment()
            },
            command: lines.command.unwrap_or_else(|| id("cmd-0")),
            by: alice(),
            models,
            tools,
            home: Some(&self.home),
            sandbox: opening.sandbox.as_deref(),
            sandbox_cache: opening.sandbox_cache,
            lineage: lines.lineage,
            sessions: lines.sessions,
            jobs: &self.jobs,
        });
        within("造会话", created).await.expect("造得出会话")
    }

    /// 载入会话 `session`，请求模型的端口由 `models` 造，工具目录是空的。
    pub async fn load(&self, session: &SessionId, models: &dyn Models) -> Handle {
        self.load_with(session, models, &Catalog::default()).await
    }

    /// 载入会话 `session`，执行工具照目录 `tools`（施工 4-2）。
    pub async fn load_with(
        &self,
        session: &SessionId,
        models: &dyn Models,
        tools: &Catalog,
    ) -> Handle {
        self.load_at(session, models, tools, &environment().cwd)
            .await
    }

    /// 同 [`Home::load_with`]，工作目录是 `cwd`（施工 4-6 上：载入以后照样在那个工作区里干活）。
    pub async fn load_at(
        &self,
        session: &SessionId,
        models: &dyn Models,
        tools: &Catalog,
        cwd: &str,
    ) -> Handle {
        self.load_full(session, models, tools, cwd, None).await
    }

    /// 同 [`Home::load_at`]，造子会话的端口是 `sessions`（施工 7-5）。
    pub async fn load_full(
        &self,
        session: &SessionId,
        models: &dyn Models,
        tools: &Catalog,
        cwd: &str,
        sessions: Option<Arc<dyn SessionPort>>,
    ) -> Handle {
        let loaded = load(Load {
            root: &self.root,
            owner: alice_account(),
            id: session.clone(),
            environment: Environment {
                cwd: cwd.to_string(),
                ..environment()
            },
            models,
            tools,
            home: Some(&self.home),
            // 载入以后的测试不执行命令：沙盒用不了。
            sandbox: None,
            sandbox_cache: None,
            sessions,
            jobs: &self.jobs,
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
        dirs: Vec::new(),
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
