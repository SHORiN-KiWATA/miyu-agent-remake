//! 替身里别的会话发来的话（施工 C-2，`docs/blueprint/cross-session.md` 第四条）：和子代理的留言是同一个命令，`by` 是一个
//! 会话；它是不是别的会话，由会话照日志认（既不是父会话、也不是派的子代理）。
//!
//! 「空了告诉我」（施工 C-6，第六条）：被等的会话交来的通知（命令 `PeerIdle`），执行器交的到点、不在了（输入
//! `WatchEnded`），和执行器、被等的那一边照着办事的三样查询。订是剧本里的一次调用：[`super::Play::watches`]。

use crate::event::IdleReason;
use crate::id::{CommandId, SessionId};
use crate::origin::{By, Session};
use crate::session::{Command, Input};
use crate::time::Timestamp;

use super::Stage;

impl Stage {
    /// 会话 `session` 发来一句（施工 C-2）：`by` 是它，一块字。返回这个命令的编号。
    ///
    /// # Panics
    ///
    /// `session` 不是会话编号的写法。
    pub fn peer_says(&mut self, session: &str, words: &str) -> CommandId {
        self.child_says(session, words)
    }

    /// 同 [`Stage::peer_says`]，这个命令正好在 `at` 这一刻到：防刷屏的窗口要算到毫秒。之后的输入照旧从这一刻往后走。
    ///
    /// # Panics
    ///
    /// `session` 不是会话编号的写法；`at` 是时间范围的第一刻。
    pub fn peer_says_at(&mut self, session: &str, words: &str, at: Timestamp) -> CommandId {
        self.now = Timestamp::from_unix_millis(at.unix_millis() - 1000)
            .unwrap_or_else(|| panic!("拨出了时间的范围"));
        self.peer_says(session, words)
    }

    /// 会话 `session` 交来「空了」，带着 `status` 那一行（施工 C-6）。返回这个命令的编号。
    ///
    /// # Panics
    ///
    /// `session` 不是会话编号的写法。
    pub fn peer_idle(&mut self, session: &str, status: Option<&str>) -> CommandId {
        let id = SessionId::parse(session).unwrap_or_else(|e| panic!("会话编号的写法坏了：{e}"));
        let status = status.map(str::to_string);
        self.command_as(By::Session(Session { id }), Command::PeerIdle { status })
    }

    /// 执行器交来：等会话 `session` 等不到了，`reason` 是 `expired` 或者 `gone`，到的时刻照替身的时钟（施工 C-6）。
    ///
    /// # Panics
    ///
    /// `session` 不是会话编号的写法。
    pub fn watch_ends(&mut self, session: &str, reason: IdleReason) {
        let session =
            SessionId::parse(session).unwrap_or_else(|e| panic!("会话编号的写法坏了：{e}"));
        let at = self.tick();
        self.run(Input::WatchEnded {
            at,
            session,
            reason,
        });
    }

    /// 同 [`Stage::watch_ends`]，正好在 `at` 这一刻到：作废要算到毫秒。之后的输入照旧从这一刻往后走。
    ///
    /// # Panics
    ///
    /// `session` 不是会话编号的写法；`at` 是时间范围的第一刻。
    pub fn watch_ends_at(&mut self, session: &str, reason: IdleReason, at: Timestamp) {
        self.now = Timestamp::from_unix_millis(at.unix_millis() - 1000)
            .unwrap_or_else(|| panic!("拨出了时间的范围"));
        self.watch_ends(session, reason);
    }

    /// 在等哪几个会话、各从哪一刻算起，照内核交给执行器的那一份（施工 C-6）。
    pub fn watching(&self) -> Vec<(SessionId, Timestamp)> {
        self.session.watching()
    }

    /// 「空了」：被等的那一边照它发通知（施工 C-6）。
    pub fn vacant(&self) -> bool {
        self.session.vacant()
    }

    /// 通知里带的那一行（施工 C-6）。
    pub fn last_line(&self) -> Option<String> {
        self.session.last_line()
    }
}
