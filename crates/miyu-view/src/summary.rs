//! 收起那一行（终端蓝图 `tui.md`「时间线」第 17 条，原样搬进核心；字和网页演示的 `timeline.summary` 一字不差）：
//!
//! - 只想过：`Thought for 26s`；
//! - 一段里只有一条命令、它有短标题：短标题打头；
//! - 别的按类数：命令、子代理、留言、给会话的留言、别的工具、编辑，先有哪样哪样打头，别的类依次跟在后面；
//! - 编辑那一格后面接加减的总行数，出错的那一格单拎出来；末尾接这一段的用时。

use crate::entry::{Part, Tone};
use crate::words::{self, ToolKind, Words, keys};

/// 一步在收起那一行里算什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Counted {
    /// 一段思考，想了多久。
    Thought(u64),
    /// 一步工具。
    Tool {
        /// 算哪一类；别的工具是 `None`。
        kind: Option<ToolKind>,
        /// 出错、被拒。
        failed: bool,
        /// 编辑加减的行数。
        diff: (u64, u64),
        /// 命令的短标题。
        title: Option<String>,
        /// 留言发给的是别的会话（编号的写法）。
        to_session: bool,
    },
}

/// 一段里各类的数。
#[derive(Debug, Default, PartialEq, Eq)]
struct Tally {
    commands: usize,
    edits: usize,
    tools: usize,
    agents: usize,
    messages: usize,
    session_messages: usize,
    thoughts: usize,
    errors: usize,
    thinking_ms: u64,
    added: u64,
    removed: u64,
    command_title: Option<String>,
    lone: bool,
}

impl Tally {
    fn count(steps: &[Counted]) -> Tally {
        let mut tally = Tally::default();
        for step in steps {
            match step {
                Counted::Thought(ms) => {
                    tally.thoughts += 1;
                    tally.thinking_ms += ms;
                }
                Counted::Tool {
                    kind,
                    failed,
                    diff,
                    to_session,
                    ..
                } => {
                    if *failed {
                        tally.errors += 1;
                    }
                    match kind {
                        Some(ToolKind::Command) => tally.commands += 1,
                        // 出错、被拒的编辑没改成：算成用过一件工具、一个出错，不算编辑、不算行数。
                        Some(ToolKind::Edit) if *failed => tally.tools += 1,
                        Some(ToolKind::Edit) => {
                            tally.edits += 1;
                            tally.added += diff.0;
                            tally.removed += diff.1;
                        }
                        Some(ToolKind::Agent) => tally.agents += 1,
                        Some(ToolKind::Message) if *to_session => tally.session_messages += 1,
                        Some(ToolKind::Message) => tally.messages += 1,
                        None => tally.tools += 1,
                    }
                }
            }
        }
        tally.lone = tally.doers() == 1;
        if tally.commands == 1 {
            tally.command_title = steps.iter().find_map(|step| match step {
                Counted::Tool {
                    kind: Some(ToolKind::Command),
                    title,
                    ..
                } => title.clone().filter(|t| !t.trim().is_empty()),
                _ => None,
            });
        }
        tally
    }

    /// 做事的有几件：思考不算。
    fn doers(&self) -> usize {
        self.commands
            + self.edits
            + self.tools
            + self.agents
            + self.messages
            + self.session_messages
    }
}

/// 只有一条命令、它出错了：整行红。和别的工具、编辑同段时不整行红。
pub(crate) fn failed(steps: &[Counted]) -> bool {
    let tally = Tally::count(steps);
    tally.command_title.is_some() && tally.lone && tally.errors > 0
}

/// 打头那一格是哪一类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lead {
    Title,
    Commands,
    Agents,
    Messages,
    SessionMessages,
    Tools,
    Edits,
}

/// 收起那一行，照 `words` 的语言；`took_ms` 是这一段的用时。
pub(crate) fn line(steps: &[Counted], took_ms: u64, words: &dyn Words) -> Vec<Part> {
    let tally = Tally::count(steps);
    let key = |name: &str| format!("{}/{name}", keys::SUMMARY);
    let count = |n: usize, name: &str| words::count(words, &key(name), n);
    if tally.doers() == 0 {
        let secs = format!("{}s", (tally.thinking_ms / 1000).max(1));
        let text =
            words::say(words, keys::THOUGHT_FOR, &[("elapsed", secs.clone())]).unwrap_or(secs);
        return vec![plain(text)];
    }
    let lead = if tally.command_title.is_some() {
        Lead::Title
    } else if tally.commands > 0 {
        Lead::Commands
    } else if tally.agents > 0 {
        Lead::Agents
    } else if tally.messages > 0 {
        Lead::Messages
    } else if tally.session_messages > 0 {
        Lead::SessionMessages
    } else if tally.tools > 0 {
        Lead::Tools
    } else {
        Lead::Edits
    };
    // 一格一格：字，和它后面要不要接加减的行数。
    let mut cells: Vec<(String, bool)> = vec![match lead {
        Lead::Title => (tally.command_title.clone().unwrap_or_default(), false),
        Lead::Commands => (count(tally.commands, "ran"), false),
        Lead::Agents => (count(tally.agents, "spawned"), false),
        Lead::Messages => (count(tally.messages, "messaged"), false),
        Lead::SessionMessages => (count(tally.session_messages, "messaged-sessions"), false),
        Lead::Tools => (count(tally.tools, "used"), false),
        Lead::Edits => (count(tally.edits, "made"), true),
    }];
    if lead != Lead::Edits && tally.edits > 0 {
        cells.push((count(tally.edits, "edits"), true));
    }
    if lead != Lead::Agents && tally.agents > 0 {
        cells.push((count(tally.agents, "agents"), false));
    }
    if lead != Lead::Messages && tally.messages > 0 {
        cells.push((count(tally.messages, "messages"), false));
    }
    // 给别的会话的留言：不打头也写成 Messaged 那一种。
    if lead != Lead::SessionMessages && tally.session_messages > 0 {
        cells.push((count(tally.session_messages, "messaged-sessions"), false));
    }
    if lead != Lead::Tools && tally.tools > 0 {
        cells.push((count(tally.tools, "tools"), false));
    }
    if tally.thoughts > 0 {
        cells.push((count(tally.thoughts, "thoughts"), false));
    }
    let errors = (tally.errors > 0).then(|| count(tally.errors, "errors"));
    if let Some(errors) = &errors {
        cells.push((errors.clone(), false));
    }
    cells.push((clock(took_ms), false));
    parts(cells, errors.as_deref(), (tally.added, tally.removed))
}

/// 一格一格用 ` · ` 接起来：要接行数的那一格后面接 ` +3 -1`（都是 0 的不接），出错的那一格单拎出来。
fn parts(
    cells: Vec<(String, bool)>,
    errors: Option<&str>,
    (added, removed): (u64, u64),
) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::new();
    let mut text = String::new();
    for (i, (cell, counts)) in cells.into_iter().enumerate() {
        if i > 0 {
            text.push_str(" · ");
        }
        if errors == Some(cell.as_str()) {
            flush(&mut out, &mut text);
            out.push(toned(cell, Tone::Error));
            continue;
        }
        text.push_str(&cell);
        if counts && added + removed > 0 {
            text.push(' ');
            flush(&mut out, &mut text);
            out.push(toned(format!("+{added}"), Tone::Added));
            text.push(' ');
            flush(&mut out, &mut text);
            out.push(toned(format!("-{removed}"), Tone::Removed));
        }
    }
    flush(&mut out, &mut text);
    out
}

fn flush(out: &mut Vec<Part>, text: &mut String) {
    if !text.is_empty() {
        out.push(plain(std::mem::take(text)));
    }
}

fn plain(text: String) -> Part {
    Part { text, tone: None }
}

fn toned(text: String, tone: Tone) -> Part {
    Part {
        text,
        tone: Some(tone),
    }
}

/// 用时：四舍五入到整秒，不到一秒的写 `1s`；照运行状态行的读秒写 `12s`、`1m 05s`、`1h 02m 05s`。
pub(crate) fn clock(ms: u64) -> String {
    let secs = (ms.saturating_add(500) / 1000).max(1);
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    match (h, m) {
        (0, 0) => format!("{s}s"),
        (0, _) => format!("{m}m {s:02}s"),
        _ => format!("{h}h {m:02}m {s:02}s"),
    }
}
