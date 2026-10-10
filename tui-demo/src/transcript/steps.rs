//! 时间线的一段：她开口说话之前的那一串步骤，思考、调工具（`13-终端界面.md` 第三节）。
//!
//! 一段在进行时展开，说话了、这一轮结束了就收成一行（「说话就收起」）；人点过的照人的意思。

use std::time::{Duration, Instant};

use serde_json::Value;

use miyu_kernel::event::Said;

use crate::config::{Timeline, ToolKind};
use crate::core::ToolStatus;

/// 时间线的一段。
#[derive(Debug, Clone)]
pub struct Segment {
    /// 一步步，照先后。
    pub steps: Vec<Step>,
    /// 这一段结束了：她开口说话了，或者这一轮结束了。
    pub finished: bool,
    /// 人点过：展开还是收起。没点过的照「进行中展开、结束收起」。
    pub open: Option<bool>,
}

impl Segment {
    /// 空的一段。
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            finished: false,
            open: None,
        }
    }

    /// 现在是不是展开的：人点过的照人点的；没点过的，进行中展开，结束了照 `fold` 收不收（蓝图「时间线」第 18 条）。
    pub fn expanded(&self, fold: bool) -> bool {
        self.open.unwrap_or(!self.finished || !fold)
    }

    /// 在转圈的那一步（蓝图「时间线」第 19 条）：她还在写（思考中、准备……）的是最后那一步；都写完了，是最前面
    /// 那一个没结果的（核心不报哪一个开始跑了，排在前面的就是在跑的那个）。都有结果了是 `None`。
    pub fn active(&self) -> Option<usize> {
        let writing = self.steps.last().is_some_and(|s| match &s.kind {
            StepKind::Thought { .. } => s.busy(),
            StepKind::Tool { state, .. } => *state == ToolState::Preparing,
        });
        if writing {
            return Some(self.steps.len() - 1);
        }
        self.steps.iter().position(Step::busy)
    }

    /// 这一段结束了：还在准备、在跑的步骤，照原样留着（它们的结果还会来）；在想的停表。
    pub fn finish_at(&mut self, now: Instant) {
        self.finished = true;
        for step in &mut self.steps {
            if let StepKind::Thought { .. } = step.kind {
                step.stop_at(now);
            }
        }
    }
}

/// 一步。
#[derive(Debug, Clone)]
pub struct Step {
    /// 是什么。
    pub kind: StepKind,
    /// 什么时候开始的。
    pub started: Instant,
    /// 用了多久；还在进行的是 `None`。
    pub took: Option<Duration>,
    /// 人点过：铺开还是收起；没点过的照配置（`opened`）。
    pub open: Option<bool>,
    /// 调工具的调用编号，`message.assistant` 来了才知道；工具结果照它找回这一步。
    pub call_id: Option<String>,
    /// 结果里的图的内容哈希（读了图片文件的 `read`）：时间线把图接在这一步下面（「时间线」第 8 条）。
    pub images: Vec<String>,
    /// 提问的回答（`ask_user` 答了以后）：一道一行「问题：回答」，时间线接在这一步下面（「确认和提问的抽屉」第 6 条）。
    pub answers: Vec<String>,
}

/// 一步的种类。
#[derive(Debug, Clone)]
pub enum StepKind {
    /// 思考，带着想的字。
    Thought {
        /// 想的字。
        text: String,
    },
    /// 调一件工具。
    Tool {
        /// 工具名，例如 `shell`。
        name: String,
        /// 参数的 JSON，边收边接。
        args: String,
        /// 参数收齐以后读成的值；读不懂的是 `Null`。
        parsed: Value,
        /// 进行到哪了。
        state: ToolState,
        /// 结果里的字。
        output: String,
        /// 结果那一句（`human`），标题后面照它写。
        said: Option<Said>,
    },
}

/// 一件工具进行到哪了。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolState {
    /// 她还在写参数：时间线上是「准备……」那一行。
    Preparing,
    /// 参数写完了，等结果。
    Running,
    /// 有结果了，带着状态。
    Done(ToolStatus),
}

impl Step {
    /// 现在是不是铺开全文：人点过的照人点的；没点过的照配置，思考、命令、编辑各一个开关（蓝图「时间线」第 18 条）。
    pub fn opened(&self, timeline: &Timeline) -> bool {
        self.open.unwrap_or_else(|| {
            let expand = &timeline.expand;
            match &self.kind {
                StepKind::Thought { .. } => expand.thought,
                StepKind::Tool { name, .. } => match timeline.kinds.get(name) {
                    Some(ToolKind::Command) => expand.command,
                    Some(ToolKind::Edit) => expand.edit,
                    Some(ToolKind::Agent | ToolKind::Message) | None => false,
                },
            }
        })
    }

    /// 新的一步，从现在算起。
    pub fn new(kind: StepKind) -> Self {
        Self {
            kind,
            started: Instant::now(),
            took: None,
            open: None,
            call_id: None,
            images: Vec::new(),
            answers: Vec::new(),
        }
    }

    /// 新的一步，从 `at` 算起（补发来的照事件的时刻，`clock.rs`）。
    pub fn starting(kind: StepKind, at: Instant) -> Self {
        Self {
            started: at,
            ..Self::new(kind)
        }
    }

    /// 在 `now` 停表。停过的不再动。
    pub fn stop_at(&mut self, now: Instant) {
        self.took
            .get_or_insert_with(|| now.saturating_duration_since(self.started));
    }

    /// 还在进行：要转圈。
    pub fn busy(&self) -> bool {
        match &self.kind {
            StepKind::Thought { .. } => self.took.is_none(),
            StepKind::Tool { state, .. } => !matches!(state, ToolState::Done(_)),
        }
    }

    /// 出错了：图标换成叉，字变红。认 `error` 和 `denied`（被拒：只读时的写入、确认时不允许的；2026-09-30 项目主人：
    /// 写入被拒和写成了看起来一模一样）；打断、跳过不算坏。
    pub fn failed(&self) -> bool {
        matches!(
            &self.kind,
            StepKind::Tool {
                state: ToolState::Done(ToolStatus::Error | ToolStatus::Denied),
                ..
            }
        )
    }

    /// 用了多久：停了的照停表，还在进行的照到现在。
    pub fn elapsed(&self) -> Duration {
        self.took.unwrap_or_else(|| self.started.elapsed())
    }

    /// 参数里的一个字符串，没有的是 `None`。
    pub fn arg(&self, key: &str) -> Option<&str> {
        match &self.kind {
            StepKind::Tool { parsed, .. } => parsed[key].as_str(),
            StepKind::Thought { .. } => None,
        }
    }
}

/// 收起时那一行要的数：几条命令、几处编辑、几件别的工具、几段思考、几个出错。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    /// 执行命令。
    pub commands: usize,
    /// 编辑、写文件。
    pub edits: usize,
    /// 别的工具。
    pub tools: usize,
    /// 派子代理。
    pub agents: usize,
    /// 给子代理（和父会话）留言。
    pub messages: usize,
    /// 给别的会话留言（`to` 是会话编号，核心 C-5）。
    pub session_messages: usize,
    /// 思考。
    pub thoughts: usize,
    /// 出错。
    pub errors: usize,
    /// 思考加起来用了多久。
    pub thinking: Duration,
    /// 这一段从第一步开始到最后一步结束的实际时长（收起那一行末尾写它，`tui.md`「时间线」第 17 条）。
    pub span: Duration,
    /// 这一段做事的只有一件（命令、编辑、别的工具、子代理、留言合起来算一件）。
    pub lone: bool,
    /// 一段里只有一条命令、它有短标题时，那条命令的短标题：收起那一行打头用它；和编辑、别的工具同段也用
    /// （蓝图「时间线」第 17 条，2026-10-02 项目主人定）。
    pub command_title: Option<String>,
}

impl Tally {
    /// 数一段。`kind_of` 说一件工具算哪一类；没有的算工具。
    pub fn count(segment: &Segment, kind_of: impl Fn(&str) -> Option<ToolKind>) -> Self {
        let mut tally = Tally::default();
        for step in &segment.steps {
            if step.failed() {
                tally.errors += 1;
            }
            match &step.kind {
                StepKind::Thought { .. } => {
                    tally.thoughts += 1;
                    tally.thinking += step.elapsed();
                }
                StepKind::Tool { name, .. } => match kind_of(name) {
                    Some(ToolKind::Command) => tally.commands += 1,
                    // 出错、被拒的编辑没改成：算成用过一件工具、一个出错，不算编辑（「时间线」收起那一行）。
                    Some(ToolKind::Edit) if step.failed() => tally.tools += 1,
                    Some(ToolKind::Edit) => tally.edits += 1,
                    Some(ToolKind::Agent) => tally.agents += 1,
                    Some(ToolKind::Message)
                        if step
                            .arg("to")
                            .is_some_and(crate::session_list::looks_like_session) =>
                    {
                        tally.session_messages += 1;
                    }
                    Some(ToolKind::Message) => tally.messages += 1,
                    None => tally.tools += 1,
                },
            }
        }
        let first = segment.steps.iter().map(|s| s.started).min();
        let end = segment.steps.iter().map(|s| s.started + s.elapsed()).max();
        if let (Some(first), Some(end)) = (first, end) {
            tally.span = end.saturating_duration_since(first);
        }
        // 思考不算做事：先想一下再跑一条命令，照样写这条命令的短标题（项目主人选的 A）。
        let doers = tally.commands
            + tally.edits
            + tally.tools
            + tally.agents
            + tally.messages
            + tally.session_messages;
        tally.lone = doers == 1;
        // 一段里只有一条命令、它有短标题就写它：和编辑、别的工具同段也写（2026-10-02 项目主人定）。
        // 同段别的工具也可能带 `description`（派子代理），要认准命令那一步。
        if tally.commands == 1 {
            tally.command_title = segment
                .steps
                .iter()
                .find_map(|step| match &step.kind {
                    StepKind::Tool { name, .. } if kind_of(name) == Some(ToolKind::Command) => {
                        step.arg("description").filter(|s| !s.trim().is_empty())
                    }
                    _ => None,
                })
                .map(str::to_string);
        }
        tally
    }
}
