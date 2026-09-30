//! 替身里别的会话发来的话（施工 C-2，`docs/blueprint/cross-session.md` 第四条）：和子代理的留言是同一个命令，`by` 是一个
//! 会话；它是不是别的会话，由会话照日志认（既不是父会话、也不是派的子代理）。

use crate::id::CommandId;
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
}
