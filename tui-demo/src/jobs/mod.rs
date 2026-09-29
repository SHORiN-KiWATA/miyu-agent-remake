//! 后台任务表和待办（蓝图 `tui.md`「后台命令、子代理和侧边栏」）：核心推来的会话状态里的这两样。
//! 核心还没有，现在由 `fake.rs` 的假数据源推。

mod fake;

use std::time::{Duration, Instant};

pub use fake::{Feed, Script};

/// 后台任务的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    /// 放到后台的命令。
    Shell,
    /// 子代理。
    Agent,
}

/// 后台任务到哪一步了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    /// 还在跑。
    Running,
    /// 做完了（命令的退出码是 0、子代理回报了）。
    Done,
    /// 命令失败，带退出码。
    Failed(i32),
    /// 人停掉的。
    Stopped,
}

/// 后台任务表的一项。
#[derive(Debug, Clone)]
pub struct Job {
    /// 编号。
    pub id: u64,
    /// 种类。
    pub kind: JobKind,
    /// 标题：命令本身、子代理的名字。
    pub title: String,
    /// 到哪一步了。
    pub state: JobState,
    /// 开始的时刻。
    pub started: Instant,
    /// 结束的时刻。
    pub ended: Option<Instant>,
    /// 命令的输出，一行一条。
    pub output: Vec<String>,
    /// 子代理正在做什么：它最新一步的一句话。
    pub doing: String,
    /// 用掉的 token。
    pub tokens: u64,
    /// 子代理交回来的报告；还没交是空的。
    pub report: String,
}

impl Job {
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

/// 会话状态里的后台任务表和待办。
#[derive(Debug, Default)]
pub struct Board {
    /// 后台任务，照开始的先后。
    pub jobs: Vec<Job>,
    /// 待办，照模型写的先后。
    pub todos: Vec<Todo>,
}

impl Board {
    /// 在跑的后台命令有几条。
    pub fn shells_running(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.kind == JobKind::Shell && j.running())
            .count()
    }

    /// 后台面板列的命令：在跑的在前，结束了的在后，各自照开始的先后。
    pub fn shells(&self) -> Vec<&Job> {
        let shells = self.jobs.iter().filter(|j| j.kind == JobKind::Shell);
        let (running, ended): (Vec<&Job>, Vec<&Job>) = shells.partition(|j| j.running());
        running.into_iter().chain(ended).collect()
    }

    /// 在跑的子代理，照开始的先后。
    pub fn agents(&self) -> Vec<&Job> {
        self.jobs
            .iter()
            .filter(|j| j.kind == JobKind::Agent && j.running())
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

    /// 停掉一条在跑的后台任务；它是在跑的才停，返回停没停。
    pub fn stop(&mut self, id: u64, now: Instant) -> bool {
        match self.jobs.iter_mut().find(|j| j.id == id && j.running()) {
            Some(job) => {
                job.state = JobState::Stopped;
                job.ended = Some(now);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests;
