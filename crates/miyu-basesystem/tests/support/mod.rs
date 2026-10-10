//! 读的三件工具共用的测试场地：一个用完就删的临时目录，里面有假的家 `home/`、工作区 `work/`、数据根 `data/`。
//! 调工具时工作目录是 `work/`。

#![allow(dead_code, reason = "三份测试各用其中几样")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use miyu_kernel::block::Block;
use miyu_kernel::event::{Event, Said};
use miyu_kernel::id::JobId;
use miyu_sandbox::Sandboxed;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_tool::{
    AgentPort, Background, Call, Done, JobPort, Log, MessagePort, Progress, QuestionPort, ReadLog,
    Seen, SessionsPort, Stop, Tool, UsagePort,
};

/// 源码树里的资源目录。
pub fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 从资源目录造出来的一件工具。
pub fn tool(name: &str) -> Arc<dyn Tool> {
    let tools = miyu_basesystem::tools(&resources()).expect("资源目录里的字读得出来");
    tools
        .into_iter()
        .find(|tool| tool.spec().name == name)
        .unwrap_or_else(|| panic!("有 {name}"))
}

/// 一个用完就删的临时目录。`now` 是造它的那一刻：设修改时间都从它往回算，秒数一样的时间就一样。
pub struct Site(pub PathBuf, SystemTime);

impl Site {
    pub fn new() -> Site {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-base-{}-{}-{n}", std::process::id(), stamp()));
        for sub in ["home", "work", "data"] {
            std::fs::create_dir_all(dir.join(sub)).expect("建得了目录");
        }
        Site(dir, SystemTime::now())
    }

    /// 在场地里写一个文件，上级目录不在就建。
    pub fn file(&self, path: &str, bytes: &[u8]) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, bytes).expect("写得进");
    }

    /// 把场地里的 `path` 的修改时间设成造场地那一刻的 `seconds` 秒以前：新的在前靠它排，不靠写文件的先后。
    pub fn aged(&self, path: &str, seconds: u64) {
        let file = std::fs::File::options()
            .write(true)
            .open(self.0.join(path))
            .expect("打得开");
        file.set_modified(self.1 - Duration::from_secs(seconds))
            .expect("设得了修改时间");
    }

    /// 场地里的一处，换成真实的位置（得存在）。
    pub fn real(&self, path: &str) -> PathBuf {
        std::fs::canonicalize(self.0.join(path)).expect("在")
    }

    /// 在 `work/` 里调一次工具 `name`，参数是 `args`：交回出没出错、给她看的字。
    pub async fn call(&self, name: &str, args: serde_json::Value) -> (bool, String) {
        self.call_in("work", name, args).await
    }

    /// 在场地里的 `cwd` 这个工作目录里调一次工具。
    pub async fn call_in(&self, cwd: &str, name: &str, args: serde_json::Value) -> (bool, String) {
        let Done { error, blocks, .. } = self.done_in(cwd, name, args).await;
        let text = blocks
            .iter()
            .map(|block| match block {
                Block::Text(text) => text.text.clone(),
                other => panic!("只该有字：{other:?}"),
            })
            .collect();
        (error, text)
    }

    /// 在 `work/` 里调一次工具，交回它交的全部（施工 4-5 上：要看给人看的说法）。
    pub async fn done(&self, name: &str, args: serde_json::Value) -> Done {
        self.done_in("work", name, args).await
    }

    /// 在场地里的 `cwd` 这个工作目录里调一次工具，交回它交的全部。她什么都没看过。
    pub async fn done_in(&self, cwd: &str, name: &str, args: serde_json::Value) -> Done {
        self.done_seen(cwd, name, args, Seen::new()).await
    }

    /// 同 [`Site::done_in`]，她看过的是 `seen`（施工 4-6 上：写的工具改之前照它核对）。
    pub async fn done_seen(
        &self,
        cwd: &str,
        name: &str,
        args: serde_json::Value,
        seen: Seen,
    ) -> Done {
        self.done_with(cwd, name, args, seen, Stop::default()).await
    }

    /// 同 [`Site::done_seen`]，交给工具的旗是 `stop`（施工 4-9 再补一：举了旗的，写的工具不改）。
    pub async fn done_with(
        &self,
        cwd: &str,
        name: &str,
        args: serde_json::Value,
        seen: Seen,
        stop: Stop,
    ) -> Done {
        let call = self.call_for(cwd, args, seen, stop);
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，关进沙盒 `sandboxed`（施工 5-1）。
    pub async fn done_sandboxed(
        &self,
        name: &str,
        args: serde_json::Value,
        sandboxed: Sandboxed,
    ) -> Done {
        let call = Call {
            sandbox: Some(Arc::new(sandboxed)),
            ..self.call_for("work", args, Seen::new(), Stop::default())
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，交给它这个会话的日志 `events`（施工 6-4：`history` 读它）。
    pub async fn done_with_log(
        &self,
        name: &str,
        args: serde_json::Value,
        events: Vec<Event>,
    ) -> Done {
        let call = Call {
            log: Some(Log::new(OneSegment(events))),
            ..self.call_for("work", args, Seen::new(), Stop::default())
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，派子代理的端口是 `agents`（施工 7-5：`agent` 经它派）。
    pub async fn done_with_agents(
        &self,
        name: &str,
        args: serde_json::Value,
        agents: Option<Arc<dyn AgentPort>>,
    ) -> Done {
        let call = Call {
            agents,
            ..self.call_for("work", args, Seen::new(), Stop::default())
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，发话的端口是 `messages`（施工 7-7、C-5：`send_message` 经它送）。
    pub async fn done_with_messages(
        &self,
        name: &str,
        args: serde_json::Value,
        messages: Option<Arc<dyn MessagePort>>,
    ) -> Done {
        let call = Call {
            messages,
            ..self.call_for("work", args, Seen::new(), Stop::default())
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，任务端口是 `jobs`，关进沙盒 `sandbox`（施工 7-3：后台命令交给它）。
    pub async fn done_jobs(
        &self,
        name: &str,
        args: serde_json::Value,
        jobs: Option<Arc<dyn JobPort>>,
        sandbox: Option<Sandboxed>,
    ) -> Done {
        let call = Call {
            jobs,
            sandbox: sandbox.map(Arc::new),
            ..self.call_for("work", args, Seen::new(), Stop::default())
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，列会话的端口是 `sessions`、会话的时区是 `offset`、叫停的旗是 `stop`（施工 C-3：`sessions`
    /// 经它列）。
    pub async fn done_with_sessions(
        &self,
        name: &str,
        args: serde_json::Value,
        sessions: Option<Arc<dyn SessionsPort>>,
        offset: miyu_kernel::time::UtcOffset,
        stop: Stop,
    ) -> Done {
        let call = Call {
            sessions,
            offset,
            ..self.call_for("work", args, Seen::new(), stop)
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，查用量的端口是 `usage`、叫停的旗是 `stop`（施工 8-15：`session_usage` 经它查）。
    pub async fn done_with_usage(
        &self,
        name: &str,
        args: serde_json::Value,
        usage: Option<Arc<dyn UsagePort>>,
        stop: Stop,
    ) -> Done {
        let call = Call {
            usage,
            ..self.call_for("work", args, Seen::new(), stop)
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，看软件包的端口是 `packages`（施工 F-10 上：`packages` 经它看）。
    pub async fn done_with_packages(
        &self,
        name: &str,
        args: serde_json::Value,
        packages: Option<Arc<dyn miyu_tool::PackagesPort>>,
    ) -> Done {
        let call = Call {
            packages,
            ..self.call_for("work", args, Seen::new(), Stop::default())
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，提问的端口是 `questions`（施工 D-2：`ask_user` 经它问人）。
    pub async fn done_with_questions(
        &self,
        name: &str,
        args: serde_json::Value,
        questions: Option<Arc<dyn QuestionPort>>,
    ) -> Done {
        let call = Call {
            questions,
            ..self.call_for("work", args, Seen::new(), Stop::default())
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在 `work/` 里调一次工具，发话的端口是 `messages`、列会话的端口是 `sessions`（施工 C-5：`send_message` 的 `to`
    /// 认会话编号要两个端口一起给）。
    pub async fn done_with_messages_and_sessions(
        &self,
        name: &str,
        args: serde_json::Value,
        messages: Option<Arc<dyn MessagePort>>,
        sessions: Option<Arc<dyn SessionsPort>>,
    ) -> Done {
        let call = Call {
            messages,
            sessions,
            ..self.call_for("work", args, Seen::new(), Stop::default())
        };
        tool(name).run(call, Progress::new(|_| {})).await
    }

    /// 在场地里的 `cwd` 这个工作目录里的一次调用，不关进沙盒。
    fn call_for(&self, cwd: &str, args: serde_json::Value, seen: Seen, stop: Stop) -> Call {
        Call {
            args: args.to_string(),
            cwd: self.0.join(cwd).to_string_lossy().into_owned(),
            home: Some(self.0.join("home")),
            data_root: Some(self.0.join("data")),
            seen: Arc::new(seen),
            stop,
            sandbox: None,
            log: None,
            offset: miyu_kernel::time::UtcOffset::UTC,
            agents: None,
            messages: None,
            jobs: None,
            sessions: None,
            usage: None,
            questions: None,
            memory: None,
            packages: None,
            ids: None,
        }
    }
}

impl Drop for Site {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 照平台原生的分隔符写一条相对路径：测试里写 `/`，Windows 上换成 `\`。
pub fn native(path: &str) -> String {
    path.replace('/', std::path::MAIN_SEPARATOR_STR)
}

/// 一个真实的位置在工作目录外面时写给她看的样子：绝对路径，Windows 上去掉 `\\?\` 那个前缀。
pub fn absolute(real: &Path) -> String {
    let text = real.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) => rest.to_string(),
        None => text.into_owned(),
    }
}

/// 假的会话日志：只有一段（施工 6-4）。
struct OneSegment(Vec<Event>);

impl ReadLog for OneSegment {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        each(self.0.clone());
        Ok(())
    }
}

/// 基础系统的说法。
pub fn said(key: &str) -> Said {
    Said::new(format!("software/basesystem/{key}"))
}

/// 核对 `got` 就是 `want`，记下来，最后一起查两份字里有没有。
pub fn check(checked: &mut Vec<Said>, got: Said, want: Said) {
    assert_eq!(got, want);
    checked.push(got);
}

/// 这次调用给人看的说法。
pub fn human(done: Done) -> Said {
    done.human.expect("每一种结果都带说法")
}

/// 查过的每一种说法，中文、英文两份字里都有、换得出字；`tools` 这几件都有显示名。
pub fn readable(checked: &[Said], tools: &[&str]) {
    let root = ResourceRoot::at(resources());
    for language in ["zh", "en"] {
        let words = Human::load(&root, language).expect("给人看的字读得出来");
        for said in checked {
            assert!(words.say(said).is_some(), "{language} 没有 {said:?}");
        }
        for tool in tools {
            assert!(
                words.tool(tool).is_some(),
                "{language} 没有 {tool} 的显示名"
            );
        }
    }
}

/// 测试用的任务端口（施工 7-3）：收下交来的后台命令，编号从 `j7` 数起；`refuse` 的不收，照任务表的规矩整组杀掉。
#[derive(Default)]
pub struct Taken {
    pub commands: std::sync::Mutex<Vec<Background>>,
    pub refuse: bool,
}

impl Taken {
    /// 收下的第 `k` 条（从 0 数），拿走。
    pub fn take(&self, k: usize) -> Background {
        self.commands.lock().expect("没 panic").remove(k)
    }
}

impl JobPort for Taken {
    fn start(&self, command: Background) -> std::io::Result<JobId> {
        if self.refuse {
            command.process.kill();
            return Err(std::io::Error::other("the session stopped"));
        }
        let mut commands = self.commands.lock().expect("没 panic");
        commands.push(command);
        Ok(JobId::new(6 + commands.len() as u64).expect("从 1 数起"))
    }

    /// `shell` 用不到查和停（施工 7-4）：一个都没有。
    fn list(&self) -> Vec<miyu_tool::Listed> {
        Vec::new()
    }

    fn output(
        &self,
        _job: JobId,
    ) -> miyu_tool::Asking<'_, Result<miyu_tool::Output, miyu_tool::JobError>> {
        Box::pin(async { Err(miyu_tool::JobError::Unknown) })
    }

    fn stop(&self, _job: JobId) -> miyu_tool::Asking<'_, Result<(), miyu_tool::JobError>> {
        Box::pin(async { Err(miyu_tool::JobError::Unknown) })
    }
}

/// 纳秒时刻：临时目录名里加上它，Windows 很快复用进程号，光靠进程号和序号会撞上前一个测试进程留下的目录。
fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}
