//! 别处来的话（蓝图 `tui.md`「别处来的话」）画进正文；派后台任务的那一步的参数（「后台命令、子代理和侧边栏」：命令本身、
//! 交代的活）。

use serde_json::Value;

use super::{Kind, StepKind, Transcript};

impl Transcript {
    /// 一句不是这个界面发的话落了盘：画成一条带来处的你说的话，序号记上，开轮时照它归到那一轮。子会话开头先画上的那句
    /// 交代（序号还没有、字一样的）只补上序号，不画第二遍（「切进子会话」第 1 条）。
    pub fn foreign(&mut self, seq: Option<u64>, from: String, text: String) {
        let drawn = self.entries.iter_mut().find(|e| {
            e.kind == Kind::User
                && e.seq.is_none()
                && e.from.as_deref() == Some(from.as_str())
                && e.text == text
        });
        if let Some(entry) = drawn {
            entry.seq = seq;
            return;
        }
        self.push(Kind::User, text);
        if let Some(entry) = self.entries.last_mut() {
            entry.from = Some(from);
            entry.seq = seq;
        }
    }

    /// 另起一份空的正文，把现在这份交出来（`/new` 时旧会话还有任务在跑，停放着它）：连接、模型、限额照旧，条目编号接着往上数。
    pub fn split_off(&mut self) -> Transcript {
        let next = Transcript {
            link: self.link.clone(),
            model: self.model.clone(),
            limits: self.limits,
            next_id: self.next_id,
            ..Transcript::default()
        };
        std::mem::replace(self, next)
    }

    /// 子代理的会话：一份正文，连接照这一份，编号是它，开头先画上交代的活（带来处 `from`；订阅以前推过的看不到，
    /// 「切进子会话」第 1 条）。刚派出去的子代理马上开一轮，订阅只推订阅以后的，那一轮的 `turn.started` 多半已经
    /// 过去了：先当它在跑，状态行才认得出它正在做什么（`turn.ended` 照常收掉）。交代的活是开这一轮的，画上以后才
    /// 当它在跑，不然它会被当成排着的话。
    pub fn child(&self, session: String, from: String, task: String) -> Transcript {
        let mut child = Transcript {
            link: self.link.clone(),
            session: Some(session),
            ..Transcript::default()
        };
        if !task.is_empty() {
            child.foreign(None, from, task);
        }
        child.running = Some(std::time::Instant::now());
        child
    }

    /// 调用编号 `call_id` 那一步的参数；没有这一步、不是调工具的是 `None`。
    pub fn call_args(&self, call_id: &str) -> Option<&Value> {
        let (i, j) = *self.calls.get(call_id)?;
        let step = self.entries.get(i)?.segment.as_ref()?.steps.get(j)?;
        match &step.kind {
            StepKind::Tool { parsed, .. } => Some(parsed),
            _ => None,
        }
    }
}
