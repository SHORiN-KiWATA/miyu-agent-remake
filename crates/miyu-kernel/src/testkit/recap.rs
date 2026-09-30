//! 替身的回顾（施工 3-8 四补，`docs/blueprint/kernel/session.md`「回顾」）：要一句回顾、回顾的请求怎么回、停住的放行。回顾的
//! 请求和主请求分开记、分开排剧本：它不占主请求的剧本，也不算在 [`Stage::requests`] 里。

use super::Stage;
use super::respond::{deltas, model};
use super::script::Line;
use crate::event::Usage;
use crate::id::{CommandId, Seq};
use crate::request::Request;
use crate::session::{Command, Input};

impl Stage {
    /// 要一句回顾。返回这个命令的编号。
    pub fn recap(&mut self) -> CommandId {
        self.command(Command::Recap)
    }

    /// 回顾的请求接下来几次，照先后这样回。
    pub fn recap_model(&mut self, lines: impl IntoIterator<Item = Line>) {
        self.recap_lines.extend(lines);
    }

    /// 回顾的请求，照先后：照到第几条，和请求本身。
    pub fn recaps(&self) -> &[(Seq, Request)] {
        &self.recaps
    }

    /// 放行停住的那次回顾：送说完了。
    ///
    /// # Panics
    ///
    /// 没有停住的回顾。
    pub fn release_recap(&mut self) {
        let (upto, line) = self
            .held_recap
            .take()
            .unwrap_or_else(|| panic!("没有停住的回顾"));
        let ended = self.recap_ended(upto, &line);
        self.run(ended);
    }

    /// 回顾的请求：先报发出去了，再一块块送增量；不停住的，最后送说完了。
    pub(super) fn recap_call(&mut self, upto: Seq, request: Request) -> Vec<Input> {
        let hash = request.hash();
        self.recaps.push((upto, request));
        let line = self
            .recap_lines
            .pop_front()
            .unwrap_or_else(|| panic!("剧本里没排第 {} 次回顾说什么", self.recaps.len()));
        let mut inputs = vec![Input::RecapSent {
            at: self.tick(),
            upto,
            model: model(),
            request: hash,
        }];
        if line.error.is_none() || line.says_something() {
            for delta in deltas(&line) {
                inputs.push(Input::RecapDelta {
                    at: self.tick(),
                    upto,
                    delta,
                });
            }
        }
        if line.hold {
            self.held_recap = Some((upto, line));
        } else {
            inputs.push(self.recap_ended(upto, &line));
        }
        inputs
    }

    /// 回顾 `upto` 说完了：出错的带上分类和原话，说完了的带上用量。
    fn recap_ended(&mut self, upto: Seq, line: &Line) -> Input {
        Input::RecapEnded {
            at: self.tick(),
            upto,
            usage: line.error.is_none().then(|| {
                line.usage.unwrap_or(Usage {
                    uncached: 100,
                    cache_read: 0,
                    cache_write: 0,
                    output: 10,
                })
            }),
            error: line.error.clone(),
        }
    }
}
