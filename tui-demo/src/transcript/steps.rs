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

    /// 这一段结束了：还在准备、在跑的步骤，照原样留着（它们的结果还会来）；在想的停表。
    pub fn finish(&mut self) {
        self.finished = true;
        for step in &mut self.steps {
            if let StepKind::Thought { .. } = step.kind {
                step.stop();
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
                StepKind::Tool { name, .. } => {
                    match timeline.tools.get(name).and_then(|t| t.kind) {
                        Some(ToolKind::Command) => expand.command,
                        Some(ToolKind::Edit) => expand.edit,
                        None => false,
                    }
                }
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
        }
    }

    /// 停表。停过的不再动。
    pub fn stop(&mut self) {
        self.took.get_or_insert_with(|| self.started.elapsed());
    }

    /// 还在进行：要转圈。
    pub fn busy(&self) -> bool {
        match &self.kind {
            StepKind::Thought { .. } => self.took.is_none(),
            StepKind::Tool { state, .. } => !matches!(state, ToolState::Done(_)),
        }
    }

    /// 出错了：图标换成叉，字变红。只认 `error`；被拒、跳过这些不算坏。
    pub fn failed(&self) -> bool {
        matches!(
            &self.kind,
            StepKind::Tool {
                state: ToolState::Done(ToolStatus::Error),
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
    /// 思考。
    pub thoughts: usize,
    /// 出错。
    pub errors: usize,
    /// 思考加起来用了多久。
    pub thinking: Duration,
    /// 这一段从第一步开始到最后一步结束的实际时长（收起那一行末尾写它，`tui.md`「时间线」第 17 条）。
    pub span: Duration,
    /// 做事的只有一条命令（思考不算）时，那条命令的短标题（`shell` 的 `description`）。
    pub only_command: Option<String>,
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
                    Some(ToolKind::Edit) => tally.edits += 1,
                    _ => tally.tools += 1,
                },
            }
        }
        let first = segment.steps.iter().map(|s| s.started).min();
        let end = segment.steps.iter().map(|s| s.started + s.elapsed()).max();
        if let (Some(first), Some(end)) = (first, end) {
            tally.span = end.saturating_duration_since(first);
        }
        // 思考不算做事：先想一下再跑一条命令，照样写这条命令的短标题（项目主人选的 A）。
        let alone = tally.commands == 1 && tally.edits + tally.tools == 0;
        if alone {
            tally.only_command = segment
                .steps
                .iter()
                .find_map(|s| s.arg("description"))
                .map(str::to_string);
        }
        tally
    }
}
