//! 后台任务表和待办（蓝图 `tui.md`「后台命令、子代理和侧边栏」）：每个会话一份。后台任务照核心推来的事件记
//! （`job.started`、`job.messaged`、`job.reported`、`child.reported`）；待办核心还没有，现在由 `fake.rs` 的假数据源推。

pub mod clean;
mod fake;

use std::time::{Duration, Instant};

pub use fake::{Feed, Script};

use crate::core::{JobEnd, JobReason, JobStart};

/// 后台任务的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    /// 放到后台的命令。
    Shell,
    /// 子代理。
    Agent,
}

/// 后台面板里的一行（蓝图「后台命令、子代理和侧边栏」第 3 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelItem {
    /// 一条命令：界面里的编号。
    Job(u64),
    /// 结束了的多于一条时收起来的那一行：收了几条。
    More(usize),
    /// 展开以后最后那一行：收起。
    Less,
}

/// 后台任务到哪一步了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    /// 还在跑。
    Running,
    /// 做完了：命令退出码是 0，子代理那一轮做完了。
    Done,
    /// 命令失败，带退出码。
    Failed(i32),
    /// 命令被信号杀掉了，带信号的编号。
    Killed(u32),
    /// 被停掉了（人停的、她自己停的）。
    Stopped,
    /// 撤销派它的那一轮时停掉的。
    Undone,
    /// 核心重启、崩了停掉的。
    Restarted,
}

/// 后台任务表的一项。
#[derive(Debug, Clone)]
pub struct Job {
    /// 界面里的编号：面板、焦点照它认。
    pub id: u64,
    /// 核心的任务编号（`j2`、`j2.1`）：停掉、对上回报照它。
    pub job: String,
    /// 种类。
    pub kind: JobKind,
    /// 标题：命令本身、子代理的名字。
    pub title: String,
    /// 子代理的会话。
    pub session: Option<String>,
    /// 到哪一步了。
    pub state: JobState,
    /// 开始的时刻（收到 `job.started` 的时刻）。
    pub started: Instant,
    /// 结束的时刻。
    pub ended: Option<Instant>,
    /// 子代理正在做什么：它最新一步的一句话。
    pub doing: String,
    /// 子代理用掉的 token。
    pub tokens: u64,
    /// 子代理交回来的回答；还没交是空的。
    pub report: String,
    /// 后台命令最近读到的输出（`job.output`）；还没读过是 `None`。
    pub output: Option<crate::core::JobOutput>,
    /// 最近一次去读输出的时刻。
    pub asked: Option<Instant>,
    /// 读输出的请求发出去了、还没回来：不再发第二个。
    pub waiting: bool,
}

impl Job {
    /// 一行里写的标题：多行的命令只写第一行，后面接 ` …`（2026-09-30 项目主人：原来换行被吃掉，几行挤成一串）。
    pub fn headline(&self) -> String {
        let mut lines = self.title.trim().lines();
        let first = lines.next().unwrap_or_default().trim_end();
        if lines.next().is_some() {
            format!("{first} …")
        } else {
            first.to_string()
        }
    }

    /// 还在跑。
    pub fn running(&self) -> bool {
        self.state == JobState::Running
    }

    /// 跑了多久：结束了的算到结束那一刻。
    pub fn elapsed(&self, now: Instant) -> Duration {
        self.ended
            .unwrap_or(now)
            .saturating_duration_since(self.started)
    }
}

/// 待办的一项到哪一步了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoState {
    /// 没做。
    Pending,
    /// 在做。
    Active,
    /// 做完了。
    Done,
}

/// 待办的一项。
#[derive(Debug, Clone)]
pub struct Todo {
    /// 这一项写什么。
    pub text: String,
    /// 到哪一步了。
    pub state: TodoState,
}

/// 一个会话的后台任务表和待办。
#[derive(Debug, Default)]
pub struct Board {
    /// 后台任务，照开始的先后。
    pub jobs: Vec<Job>,
    /// 待办，照模型写的先后。
    pub todos: Vec<Todo>,
    /// 做完了、全打勾露到什么时候（`set_todos`）。
    linger: Option<Instant>,
    next_id: u64,
}

impl Board {
    /// 派出去一个（`job.started`）。`command` 是开它的那一步的命令本身，拿不到的写标题。交回界面里的编号。
    pub fn start(&mut self, start: &JobStart, command: Option<String>, now: Instant) -> u64 {
        self.next_id += 1;
        let kind = if start.agent {
            JobKind::Agent
        } else {
            JobKind::Shell
        };
        let title = match kind {
            JobKind::Shell => command
                .filter(|c| !c.trim().is_empty())
                .unwrap_or_else(|| start.title.clone()),
            JobKind::Agent => start.title.clone(),
        };
        self.jobs.push(Job {
            id: self.next_id,
            job: start.job.clone(),
            kind,
            title,
            session: start.session.clone(),
            state: JobState::Running,
            started: now,
            ended: None,
            doing: String::new(),
            tokens: 0,
            report: String::new(),
            output: None,
            asked: None,
            waiting: false,
        });
        self.next_id
    }

    /// 给报过的子代理留了言（`job.messaged`）：又在跑了，等下一份回报。
    pub fn messaged(&mut self, job: &str) {
        if let Some(j) = self.jobs.iter_mut().find(|j| j.job == job) {
            j.state = JobState::Running;
            j.ended = None;
        }
    }

    /// 结束了（`job.reported`、`child.reported`）：交回界面里的编号；对不上的（别的会话的）是 `None`。
    pub fn end(&mut self, end: &JobEnd, now: Instant) -> Option<u64> {
        let job = self.jobs.iter_mut().find(|j| j.job == end.job)?;
        job.state = match (end.reason, end.exit_code, end.signal) {
            (JobReason::Stopped, ..) => JobState::Stopped,
            (JobReason::Undone, ..) => JobState::Undone,
            (JobReason::Restarted, ..) => JobState::Restarted,
            (JobReason::Finished, Some(code), _) if code != 0 => JobState::Failed(code),
            (JobReason::Finished, None, Some(signal)) => JobState::Killed(signal),
            (JobReason::Finished, ..) => JobState::Done,
        };
        let took = end.duration_ms.map(Duration::from_millis);
        job.ended = Some(took.map_or(now, |d| job.started + d));
        if job.kind == JobKind::Agent {
            job.report.clone_from(&end.text);
        }
        Some(job.id)
    }

    /// 更早的一页里的任务接在前面（老会话按页读）：任务编号已经有的不重记，界面里的编号接着发。
    pub fn prepend(&mut self, older: Board) {
        let mut front: Vec<Job> = older
            .jobs
            .into_iter()
            .filter(|old| self.jobs.iter().all(|j| j.job != old.job))
            .collect();
        for job in &mut front {
            self.next_id += 1;
            job.id = self.next_id;
        }
        front.append(&mut self.jobs);
        self.jobs = front;
    }

    /// 还有在跑的。
    pub fn busy(&self) -> bool {
        self.jobs.iter().any(Job::running)
    }

    /// 子会话 `session` 的那个子代理的名字。
    pub fn agent_title(&self, session: &str) -> Option<&str> {
        self.jobs
            .iter()
            .find(|j| j.session.as_deref() == Some(session))
            .map(|j| j.title.as_str())
    }

    /// 子会话 `session` 的那个子代理。
    pub fn agent_mut(&mut self, session: &str) -> Option<&mut Job> {
        self.jobs
            .iter_mut()
            .find(|j| j.session.as_deref() == Some(session))
    }

    /// 核心推来的待办整份换上（`pending`、`in_progress`、`completed`，认不得的当没做）。清空时带着刚做完的（`done`）的，
    /// 先照全打勾露 `linger` 再收（2026-10-07 项目主人：做完之后保留一会）。
    pub fn set_todos(
        &mut self,
        todos: &[crate::core::TodoItem],
        done: &[crate::core::TodoItem],
        now: Instant,
        linger: Duration,
    ) {
        let lingering = todos.is_empty() && !done.is_empty();
        let shown = if lingering { done } else { todos };
        self.todos = shown
            .iter()
            .map(|t| Todo {
                text: t.content.clone(),
                state: match t.status.as_str() {
                    "in_progress" => TodoState::Active,
                    "completed" => TodoState::Done,
                    _ => TodoState::Pending,
                },
            })
            .collect();
        self.linger = lingering.then_some(now + linger);
    }

    /// 待办露不露、做了几项：没做完的露着；全做完的只在保留的那一会儿里露（`set_todos`）。
    pub fn todo_shown(&self, now: Instant) -> Option<(usize, usize)> {
        let lingering = self.linger.is_some_and(|until| now < until);
        self.todo_progress()
            .filter(|(done, total)| done < total || lingering)
    }

    /// 全打勾露到什么时候（主循环到点醒来收）。
    pub fn todos_until(&self) -> Option<Instant> {
        self.linger
    }

    /// 到点了：保留着的那一份收掉。
    pub fn expire_todos(&mut self, now: Instant) {
        if self.linger.is_some_and(|until| now >= until) {
            self.linger = None;
            self.todos.clear();
        }
    }

    /// 在跑的后台命令有几条。
    pub fn shells_running(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.kind == JobKind::Shell && j.running())
            .count()
    }

    /// 核心的任务编号是 `job` 的那一条。
    pub fn by_job_mut(&mut self, job: &str) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.job == job)
    }

    /// 后台面板里的一条一条（蓝图「后台命令、子代理和侧边栏」第 3 条）：在跑的照开始的先后；结束了的只有一条的照列，
    /// 多于一条的只露最近结束的那条，别的收成一行（2026-09-30 项目主人定），`all` 是展开了，照开始的先后全列、最后一行收起。
    pub fn panel_items(&self, all: bool) -> Vec<PanelItem> {
        let shells = self.jobs.iter().filter(|j| j.kind == JobKind::Shell);
        let (running, ended): (Vec<&Job>, Vec<&Job>) = shells.partition(|j| j.running());
        let mut out: Vec<PanelItem> = running.iter().map(|j| PanelItem::Job(j.id)).collect();
        match ended.iter().max_by_key(|j| j.ended) {
            Some(_) if ended.len() == 1 || all => {
                out.extend(ended.iter().map(|j| PanelItem::Job(j.id)));
                if ended.len() > 1 {
                    out.push(PanelItem::Less);
                }
            }
            Some(last) => {
                out.push(PanelItem::Job(last.id));
                out.push(PanelItem::More(ended.len() - 1));
            }
            None => {}
        }
        out
    }

    /// 在跑的子代理，照开始的先后。
    pub fn agents(&self) -> Vec<&Job> {
        self.jobs
            .iter()
            .filter(|j| j.kind == JobKind::Agent && j.running())
            .collect()
    }

    /// 子代理状态行列的子代理：在跑的，加上正在看的那个（报完了也留着，蓝图「切进子会话」第 5 条），照开始的先后。
    pub fn listed(&self, viewing: Option<&str>) -> Vec<&Job> {
        let viewed = |j: &Job| viewing.is_some() && j.session.as_deref() == viewing;
        self.jobs
            .iter()
            .filter(|j| j.kind == JobKind::Agent && (j.running() || viewed(j)))
            .collect()
    }

    /// 待办做完几项、一共几项；没有待办是 `None`。
    pub fn todo_progress(&self) -> Option<(usize, usize)> {
        let done = self
            .todos
            .iter()
            .filter(|t| t.state == TodoState::Done)
            .count();
        (!self.todos.is_empty()).then_some((done, self.todos.len()))
    }
}

#[cfg(test)]
mod tests;
