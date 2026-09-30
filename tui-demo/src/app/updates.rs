//! 收核心那边的消息（蓝图 `tui.md`「连核心」「正文」）：交给正文之前，界面这一头先办的几件——没发出去的撤回来、
//! 只弹提示的拒绝、撤销恢复时输入框里的那句、视口跟不跟、系统通知、被退回的排队消息放回输入框。

use super::App;
use crate::core::{Block, Push, Update};
use crate::transcript::Kind;

impl App {
    /// 收一条核心那边的消息。撤销成了、排队的消息被退回了，字放回输入框（`tui.md`「输入框」第 7、8 条）。
    pub fn core(&mut self, update: Update) {
        // 命令的输出：记进任务表，不经正文（`output.rs`）。
        if let Update::Output {
            session,
            job,
            output,
        } = update
        {
            self.took_output(&session, &job, output);
            return;
        }
        // 照会话分：别的会话的交给停放着的那一份；任务的几种、别处来的话照任务表、正文先办（`sessions.rs`）。
        let Some(update) = self.route(update) else {
            return;
        };
        if let Update::Push(push) = &update
            && self.visible_push(push)
        {
            return;
        }
        // Ctrl+C 打断、要退回排着的：这一轮结束了撤掉它，撤掉了放回输入框（`takeback.rs`）。
        self.takeback_on(&update);
        // 没发出去：先撤掉先画上的那句、字放回输入框，接着照一般的拒绝办（`redo.rs`）。
        let update = match update {
            Update::Unsent { reason, message } => {
                self.take_back_unsent();
                Update::Refused { reason, message }
            }
            update => update,
        };
        // 这几种拒绝只弹提示框，不写进正文（`tui.md`「正文」第 9 条）。
        if let Update::Refused {
            reason: Some(reason),
            ..
        } = &update
            && let Some(hint) = self.config.text.refusal_hints.get(reason)
        {
            self.hint(hint.clone(), false);
            return;
        }
        if let Update::Renamed(title) = &update {
            let texts = &self.config.text.rename;
            let note = title.as_ref().map_or_else(
                || texts.removed.clone(),
                |title| texts.done.replace("{title}", title),
            );
            self.hint(note, false);
            return;
        }
        let undone = matches!(update, Update::Undone { restore: false, .. });
        if matches!(update, Update::Undone { restore: true, .. }) {
            self.input.take_back();
        }
        // 限制了进行中那一段的高度就不放开视口（`tui.md`「正文」第 1 条、「时间线」第 20 条）。
        // 限制着的，一轮结束运行状态行收起时也按住不往下落。
        let release = crate::ui::release_on_fold(&self.config.timeline);
        if matches!(update, Update::Push(crate::core::Push::TurnEnded(_))) {
            if release {
                self.view.settle();
            } else {
                self.view.hold();
            }
        }
        // 她开始下一步：长正文替人停着的，回到最底下接着跟（`tui.md`「正文」第 1 条）。
        if let Update::Push(Push::BlockStart { block, .. }) = &update
            && *block != Block::Text
        {
            self.view.resume();
        }
        // 系统通知、报给 herdr：在正文收它之前量这一轮用了多久（「系统通知」）。
        self.notify_core(&update);
        // 清空了：像 Ctrl+L 一样清屏，「上下文已清空」在新的一屏顶上（「正文」第 9 条）。
        if matches!(update, Update::Push(Push::Compacted { clear: true })) {
            self.view.clear();
        }
        let folds = self.transcript.folds();
        self.transcript.update(update, &self.config.text);
        // 在子会话里看：它那一行正在做什么、用了多少跟着更新。
        if let Some(child) = self.viewing().map(str::to_string) {
            self.refresh_agent(&child);
        }
        // 一段刚收起（她开口、一轮结束）：放开一次视口，收起留下的空白由上面的行补满（`tui.md`「正文」第 1 条）。
        if release && self.transcript.folds() > folds {
            self.view.settle();
        }
        // 被退回的排队消息连同粘贴块放回输入框（`takeback.rs`）。
        let returned = self.transcript.take_returned();
        self.put_returned(returned);
        if undone {
            let said = self
                .transcript
                .entries
                .iter()
                .rev()
                .find(|e| e.kind == Kind::Undo);
            if let Some(said) = said.map(|e| e.text.clone()) {
                self.input.put_back(&said);
            }
        }
    }
}
