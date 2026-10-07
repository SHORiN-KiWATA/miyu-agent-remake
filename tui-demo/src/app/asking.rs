//! 核心推来的确认、提问接到抽屉上（蓝图「确认和提问的抽屉」开头一段、第 1、6、7、8 条；核心 D-1、D-2）：来了开抽屉、
//! 这次调用了结了收起；推来的回答写进正文；交了被拒的重新打开、弹一句。

use super::App;
use crate::core::{Asking, Push, Update};
use crate::drawer::{Answered, Approval, Asked, Decided, Drawer, Outcome};

impl App {
    /// 先看一眼核心的消息：确认、提问归抽屉（交回 `true`，不再往下走）；工具有了结果的收起它的抽屉（照常往下走）。
    pub(super) fn asking_update(&mut self, update: &Update) -> bool {
        match update {
            Update::Push(push) => {
                let owner = self.main_session().unwrap_or_default();
                self.asking_push(&owner, None, push)
            }
            // 推送一律带着会话编号来（`serve.rs`）：主会话的当主会话的，别的是子会话问的（第 8 条）。
            Update::Elsewhere { session, update } => match update.as_ref() {
                Update::Push(push) => {
                    let main = self.main_session().as_deref() == Some(session.as_str());
                    let asker = (!main).then(|| session.clone());
                    self.asking_push(session, asker.as_ref(), push)
                }
                _ => false,
            },
            Update::AnswerRefused {
                session,
                call,
                reason,
                message,
            } => {
                // 已经答过、了结了：不说，抽屉照推送收（第 7 条）。别的重新打开、弹核心的原话。
                if reason.as_deref() != Some("not_asking")
                    && let Some(drawer) = self.asks.again(session, call)
                {
                    self.drawers.reopen(drawer);
                    self.hint(message.clone(), false);
                }
                true
            }
            _ => false,
        }
    }

    /// `owner` 是问的会话的整个编号；`session` 只有子会话问的才有（写谁在问、结果写不写进正文）。
    fn asking_push(&mut self, owner: &str, session: Option<&String>, push: &Push) -> bool {
        match push {
            Push::Asking(asking) => {
                self.asking(owner, session, asking);
                true
            }
            Push::ToolResult { call_id, .. } => {
                self.asks.settle(owner, call_id);
                self.close_drawer(owner, call_id);
                false
            }
            _ => false,
        }
    }

    fn asking(&mut self, owner: &str, session: Option<&String>, asking: &Asking) {
        let who = session.map(|s| {
            let name = self.board.agent_title(s).unwrap_or(s);
            self.config.text.drawer.agent.replace("{name}", name)
        });
        let drawer = match asking {
            Asking::Asked(body) => serde_json::from_value::<Asked>(body.clone())
                .ok()
                .map(|a| Drawer::question(who, a)),
            Asking::Approval(body) => serde_json::from_value::<Approval>(body.clone())
                .ok()
                .map(|a| Drawer::approval(who, a)),
            Asking::Answered(body) => {
                if let Ok(a) = serde_json::from_value::<Answered>(body.clone()) {
                    self.answered(owner, session, &a.call_id.clone(), &Outcome::Answered(a));
                }
                return;
            }
            Asking::Decided(body) => {
                if let Ok(d) = serde_json::from_value::<Decided>(body.clone()) {
                    self.answered(owner, session, &d.call_id.clone(), &Outcome::Decided(d));
                }
                return;
            }
        };
        let Some(mut drawer) = drawer else {
            return;
        };
        drawer.session = session.cloned();
        drawer.owner = owner.to_string();
        // 跑命令的确认：核心的 `detail` 没带短标题、命令的（D-4 以前的核心），照时间线里这一步的参数写。
        let args = session.map_or_else(|| self.transcript.tool_args(&drawer.call_id), |_| None);
        if drawer.command.is_none()
            && let Some(command) = args.and_then(|a| a["command"].as_str())
        {
            let title = args
                .and_then(|a| a["description"].as_str())
                .map(str::to_string);
            drawer.command = Some((title.filter(|t| !t.trim().is_empty()), command.to_string()));
        }
        if !self.asks.asked(&drawer) {
            return;
        }
        let was_open = self.drawers.open();
        self.drawers.push(drawer);
        // 新开的抽屉：弹「在等你」（「系统通知」第 1 条）；前一个还没了结的排着，轮到它时再弹。
        if !was_open {
            self.notify_drawer();
        }
    }

    /// 核心落了盘的回答（哪个头答的都算）：收起抽屉；主会话的在正文末尾写结果（第 6 条）。
    fn answered(
        &mut self,
        owner: &str,
        session: Option<&String>,
        call_id: &str,
        outcome: &Outcome,
    ) {
        let asked = self.asks.settle(owner, call_id);
        self.close_drawer(owner, call_id);
        if session.is_none()
            && let Some(drawer) = asked
        {
            self.write_report(&drawer, outcome);
        }
    }

    /// 这一次调用的抽屉开着的收起、排着的拿掉；都了结了回到在做或空闲（「系统通知」第 1、6 条）。
    fn close_drawer(&mut self, owner: &str, call_id: &str) {
        let was_open = self.drawers.open();
        if self.drawers.settle(owner, call_id) {
            if self.drawers.open() {
                self.notify_drawer();
            } else if was_open {
                self.settle_state();
            }
        }
    }
}
